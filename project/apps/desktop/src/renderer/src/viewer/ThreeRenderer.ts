import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { StructureScene, Vec3 } from "@dftwork/shared";
import { elementStyle } from "./elements";
import {
  DEFAULT_OPTIONS,
  type EffectContext,
  type RenderEffect,
  type RenderOptions,
  type SceneRenderer,
} from "./SceneRenderer";

/**
 * three.js 实现的场景渲染器。
 *
 * 分工：
 * - 这个类只管「怎么画」，不碰 React、不碰数据获取；
 * - 组件只负责挂载容器、把数据递进来、卸载时 dispose。
 *
 * 坐标系：VASP 的笛卡尔坐标（Å）直接用，但整体平移使**几何中心落在原点**——
 * 否则大盒子（比如 18×12×12 的 MD 盒子）会有一半在视野外，用户一打开就懵。
 */
export class ThreeRenderer implements SceneRenderer {
  readonly name = "three";

  private container: HTMLElement | null = null;
  private renderer: THREE.WebGLRenderer | null = null;
  private scene3d: THREE.Scene | null = null;
  private camera: THREE.PerspectiveCamera | null = null;
  private controls: OrbitControls | null = null;
  private root: THREE.Group | null = null;

  private options: RenderOptions = { ...DEFAULT_OPTIONS };
  private data: StructureScene | null = null;
  private effects: RenderEffect[] = [];
  private clock = new THREE.Clock();
  private observer: ResizeObserver | null = null;
  private home = { pos: new THREE.Vector3(0, 0, 30), target: new THREE.Vector3(0, 0, 0) };

  mount(container: HTMLElement): void {
    if (this.renderer) this.dispose();
    this.container = container;

    const w = Math.max(container.clientWidth, 1);
    const h = Math.max(container.clientHeight, 1);

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setSize(w, h);
    renderer.domElement.style.display = "block";
    renderer.domElement.style.width = "100%";
    renderer.domElement.style.height = "100%";
    container.appendChild(renderer.domElement);
    this.renderer = renderer;

    const scene3d = new THREE.Scene();
    this.scene3d = scene3d;

    const camera = new THREE.PerspectiveCamera(45, w / h, 0.1, 10_000);
    camera.position.copy(this.home.pos);
    this.camera = camera;

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.09;
    controls.rotateSpeed = 0.85;
    controls.zoomSpeed = 0.9;
    this.controls = controls;

    // 三盏灯：主光给出立体感，补光让背面不至于全黑，底光把下方原子提亮
    scene3d.add(new THREE.AmbientLight(0xffffff, 1.15));
    const key = new THREE.DirectionalLight(0xffffff, 1.5);
    key.position.set(1, 1.2, 1.8);
    const fill = new THREE.DirectionalLight(0xffffff, 0.65);
    fill.position.set(-1.4, -0.9, -1.1);
    const bottom = new THREE.DirectionalLight(0xffffff, 0.45);
    bottom.position.set(0, -1.6, 0.4);
    scene3d.add(key, fill, bottom);

    const root = new THREE.Group();
    scene3d.add(root);
    this.root = root;

    renderer.setAnimationLoop(() => {
      const dt = this.clock.getDelta();
      this.controls?.update();
      for (const e of this.effects) e.update?.(dt);
      renderer.render(scene3d, camera);
    });

    // 容器尺寸变化（窗口缩放、面板拖拽）时自动跟上
    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(container);
  }

  setScene(data: StructureScene): void {
    const root = this.root;
    const camera = this.camera;
    const controls = this.controls;
    if (!root || !camera || !controls) return;

    this.data = data;
    this.clearSafely(root);
    this.applyBackground();

    if (data.atoms.length === 0) {
      this.renderer?.render(this.scene3d!, camera);
      return;
    }

    // ── 1. 几何中心：原子 + 晶胞顶点一起算，保证盒子四个角都在视野里
    const corners = cellCorners(data.cell);
    const points = [...corners, ...data.atoms.map((a) => new THREE.Vector3(a.position[0], a.position[1], a.position[2]))];
    const box = new THREE.Box3();
    for (const p of points) box.expandByPoint(p);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z) || 1;

    // ── 2. 原子球：同元素共用材质，几何体全场共用一个单位球
    const sphere = new THREE.SphereGeometry(1, 28, 20);
    const matByElement = new Map<string, THREE.MeshStandardMaterial>();
    const atomPositions: THREE.Vector3[] = [];

    for (const atom of data.atoms) {
      const style = elementStyle(atom.element);
      let mat = matByElement.get(atom.element);
      if (!mat) {
        mat = new THREE.MeshStandardMaterial({
          color: new THREE.Color(style.color),
          roughness: 0.36,
          metalness: 0.14,
        });
        matByElement.set(atom.element, mat);
      }
      const mesh = new THREE.Mesh(sphere, mat);
      const r = Math.max(style.radius * this.options.atomScale, 0.06);
      mesh.scale.setScalar(r);
      const p = new THREE.Vector3(atom.position[0], atom.position[1], atom.position[2]).sub(center);
      mesh.position.copy(p);
      atomPositions.push(p);
      root.add(mesh);
    }

    // ── 3. 晶胞棱
    if (this.options.showCell) {
      root.add(buildCellLines(corners, center, this.options.cellColor));
    }

    // ── 4. 化学键（首版 bonds 为空，等成键算法上线后自动生效）
    if (this.options.showBonds && data.bonds.length > 0) {
      root.add(buildBonds(data.bonds, atomPositions));
    }

    // ── 5. 相机自适应：按包围盒外接球算距离，45° 视场下留 1.55 倍余量
    const fovRad = (camera.fov * Math.PI) / 180;
    const radius = Math.sqrt(size.x ** 2 + size.y ** 2 + size.z ** 2) / 2 || maxDim / 2;
    const dist = (radius / Math.tan(fovRad / 2)) * 1.55;

    camera.position.set(dist * 0.62, dist * 0.48, dist * 0.78);
    camera.near = Math.max(dist / 5000, 0.01);
    camera.far = dist * 30;
    camera.updateProjectionMatrix();

    controls.target.set(0, 0, 0);
    controls.minDistance = Math.max(radius * 0.08, 0.5);
    controls.maxDistance = dist * 8;
    controls.update();

    this.home = { pos: camera.position.clone(), target: new THREE.Vector3(0, 0, 0) };

    // ── 6. 自定义效果挂载点
    const ctx: EffectContext = { root, scene: data, options: this.options };
    for (const e of this.effects) e.apply(ctx);
  }

  setOptions(partial: Partial<RenderOptions>): void {
    const geomAffecting: Array<keyof RenderOptions> = ["atomScale", "showCell", "showBonds", "cellColor"];
    const needsRebuild = geomAffecting.some((k) => partial[k] !== undefined);
    this.options = { ...this.options, ...partial };
    this.applyBackground();
    if (needsRebuild && this.data) this.setScene(this.data);
  }

  resize(): void {
    const c = this.container;
    const r = this.renderer;
    const cam = this.camera;
    if (!c || !r || !cam) return;
    const w = Math.max(c.clientWidth, 1);
    const h = Math.max(c.clientHeight, 1);
    r.setSize(w, h, false);
    cam.aspect = w / h;
    cam.updateProjectionMatrix();
  }

  resetView(): void {
    const cam = this.camera;
    const controls = this.controls;
    if (!cam || !controls) return;
    cam.position.copy(this.home.pos);
    controls.target.copy(this.home.target);
    controls.update();
  }

  addEffect(effect: RenderEffect): void {
    this.effects.push(effect);
    if (this.data && this.root) {
      effect.apply({ root: this.root, scene: this.data, options: this.options });
    }
  }

  dispose(): void {
    this.observer?.disconnect();
    this.observer = null;

    if (this.renderer) {
      this.renderer.setAnimationLoop(null);
      this.renderer.dispose();
      const el = this.renderer.domElement;
      if (el.parentElement) el.parentElement.removeChild(el);
      this.renderer = null;
    }
    this.controls?.dispose();
    this.controls = null;

    if (this.root) this.clearSafely(this.root);
    for (const e of this.effects) e.dispose();
    this.effects = [];

    this.scene3d = null;
    this.camera = null;
    this.root = null;
    this.container = null;
    this.data = null;
  }

  // ── 内部工具 ────────────────────────────────────────────

  private applyBackground(): void {
    const bg = this.options.background;
    if (!this.renderer) return;
    if (bg === "transparent") {
      this.renderer.setClearColor(0x000000, 0);
    } else {
      this.renderer.setClearColor(new THREE.Color(bg), 1);
    }
  }

  /** 清子对象并释放显存。注意共享几何体/材质会被多次 dispose —— three 对此是幂等的。 */
  private clearSafely(group: THREE.Group): void {
    for (const child of [...group.children]) {
      group.remove(child);
      child.traverse((node) => {
        const mesh = node as THREE.Mesh;
        mesh.geometry?.dispose();
        const mat = mesh.material as THREE.Material | THREE.Material[] | undefined;
        if (Array.isArray(mat)) mat.forEach((m) => m.dispose());
        else mat?.dispose();
      });
    }
  }
}

/** 晶胞的 8 个顶点，顺序按 (i,j,k) 二进制：000,001,010,011,100,101,110,111。 */
function cellCorners(cell: [Vec3, Vec3, Vec3]): THREE.Vector3[] {
  const [a, b, c] = cell.map((v) => new THREE.Vector3(v[0], v[1], v[2]));
  const out: THREE.Vector3[] = [];
  for (let i = 0; i < 2; i += 1) {
    for (let j = 0; j < 2; j += 1) {
      for (let k = 0; k < 2; k += 1) {
        out.push(
          new THREE.Vector3()
            .addScaledVector(a, i)
            .addScaledVector(b, j)
            .addScaledVector(c, k),
        );
      }
    }
  }
  return out;
}

/** 12 条棱：顶点索引只有一位不同就连一条。 */
function buildCellLines(corners: THREE.Vector3[], center: THREE.Vector3, color: string): THREE.LineSegments {
  const pts: number[] = [];
  for (let i = 0; i < 8; i += 1) {
    for (let j = i + 1; j < 8; j += 1) {
      if (popcount3(i ^ j) !== 1) continue;
      const p = corners[i].clone().sub(center);
      const q = corners[j].clone().sub(center);
      pts.push(p.x, p.y, p.z, q.x, q.y, q.z);
    }
  }
  const geom = new THREE.BufferGeometry();
  geom.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
  const mat = new THREE.LineBasicMaterial({ color: new THREE.Color(color), transparent: true, opacity: 0.85 });
  return new THREE.LineSegments(geom, mat);
}

function popcount3(n: number): number {
  let c = 0;
  let v = n;
  while (v) {
    c += v & 1;
    v >>= 1;
  }
  return c;
}

/** 化学键：每根键是一个细圆柱，从 a 指向 b。 */
function buildBonds(
  bonds: Array<{ a: number; b: number }>,
  positions: THREE.Vector3[],
): THREE.Group {
  const group = new THREE.Group();
  const cyl = new THREE.CylinderGeometry(1, 1, 1, 10, 1, true);
  const mat = new THREE.MeshStandardMaterial({ color: 0xb8c2cf, roughness: 0.55, metalness: 0.1 });
  const up = new THREE.Vector3(0, 1, 0);

  for (const bond of bonds) {
    const pa = positions[bond.a];
    const pb = positions[bond.b];
    if (!pa || !pb) continue;
    const dir = new THREE.Vector3().subVectors(pb, pa);
    const len = dir.length();
    if (len < 1e-6) continue;

    const mesh = new THREE.Mesh(cyl, mat);
    mesh.position.copy(pa).addScaledVector(dir, 0.5);
    mesh.scale.set(0.11, len, 0.11);
    mesh.quaternion.setFromUnitVectors(up, dir.normalize());
    group.add(mesh);
  }
  return group;
}
