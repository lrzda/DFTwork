import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { StructureScene, Vec3 } from "@dftwork/shared";
import { elementStyle } from "./elements";
import {
  invert3x3,
  latticeMatrix,
  minImageOffset,
  mulMatVec,
  toFractional,
  wrapToCell,
} from "./geometry";
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

    // ── 1. 有效坐标：可能落在盒子外（MD 轨迹、优化后的 CONTCAR 都常见）。
    //      wrap 开关打开时先折回原胞——必须在算包围盒之前做，
    //      否则盒外的原子会把包围盒撑大，相机拉远后盒子缩成一小块。
    const effective: Vec3[] = data.atoms.map((a) =>
      this.options.wrapPbc ? wrapToCell(a.position, data.cell) : a.position,
    );

    // 分数坐标：成键判定的镜像搜索、以及跨界镜像球定位都要用
    const invLattice = invert3x3(latticeMatrix(data.cell));
    const frac: Vec3[] = invLattice
      ? effective.map((p) => toFractional(invLattice, p))
      : effective.map(() => [0, 0, 0] as Vec3);

    // 几何中心：原子 + 晶胞顶点一起算，保证盒子四个角都在视野里
    const corners = cellCorners(data.cell);
    const points = [...corners, ...effective.map((p) => new THREE.Vector3(p[0], p[1], p[2]))];
    const box = new THREE.Box3();
    for (const p of points) box.expandByPoint(p);
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z) || 1;

    // ── 2. 原子球：同元素共用材质，几何体全场共用一个单位球
    const sphere = new THREE.SphereGeometry(1, 28, 20);
    const matByElement = new Map<string, THREE.MeshStandardMaterial>();
    const atomPositions: THREE.Vector3[] = [];

    for (const [i, atom] of data.atoms.entries()) {
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
      const p = new THREE.Vector3(effective[i][0], effective[i][1], effective[i][2]).sub(center);
      mesh.position.copy(p);
      atomPositions.push(p);
      root.add(mesh);
    }

    // ── 3. 晶胞棱
    if (this.options.showCell) {
      root.add(buildCellLines(corners, center, this.options.cellColor));
    }

    // ── 4. 化学键：跨界的键补镜像原子，否则键会横穿整个盒子
    //      （实测某 MD 快照 一批键里 107 根是跨界的，最长一根画出来 24 Å，
    //       而真实键长只有 3 Å。不补镜像的话画面几乎全是乱线。）
    if (this.options.showBonds && data.bonds.length > 0) {
      root.add(
        buildBonds({
          data,
          frac,
          atomPositions,
          center,
          sphere,
          matFor: (el) => matByElement.get(el)!,
          atomScale: this.options.atomScale,
        }),
      );
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
    const geomAffecting: Array<keyof RenderOptions> = ["atomScale", "showCell", "showBonds", "cellColor", "wrapPbc"];
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

interface BondBuildInput {
  data: StructureScene;
  /** 每个原子的分数坐标（已考虑 wrap 开关）。 */
  frac: Vec3[];
  /** 每个原子的显示位置（已减去几何中心）。 */
  atomPositions: THREE.Vector3[];
  center: THREE.Vector3;
  /** 复用的单位球几何体，用来画镜像原子。 */
  sphere: THREE.SphereGeometry;
  matFor: (element: string) => THREE.MeshStandardMaterial;
  atomScale: number;
}

/**
 * 化学键渲染。
 *
 * 同一像内的键直接连两根原子；**跨界的键改为各连一个镜像球**——
 * 这是 VESTA 的做法：与其把键拉成横穿盒子的一条长线（视觉上完全误导），
 * 不如在盒子外补一个对称位置的球，让两边的成键环境都完整。
 *
 * 镜像球的位置由 `minImageOffset` 给出：
 * - B 靠近 A 的镜像在 `fb + offset`
 * - A 靠近 B 的镜像在 `fa - offset`
 */
function buildBonds(input: BondBuildInput): THREE.Group {
  const { data, frac, atomPositions, center, sphere, matFor, atomScale } = input;

  const group = new THREE.Group();
  const cyl = new THREE.CylinderGeometry(1, 1, 1, 10, 1, true);
  const bondMat = new THREE.MeshStandardMaterial({
    color: 0xb8c2cf,
    roughness: 0.55,
    metalness: 0.1,
  });
  const up = new THREE.Vector3(0, 1, 0);
  const lattice = latticeMatrix(data.cell);

  const toCart = (f: Vec3): THREE.Vector3 => {
    const c = mulMatVec(lattice, f);
    return new THREE.Vector3(c[0], c[1], c[2]).sub(center);
  };

  const addStick = (p0: THREE.Vector3, p1: THREE.Vector3): void => {
    const dir = new THREE.Vector3().subVectors(p1, p0);
    const len = dir.length();
    if (len < 1e-6 || !Number.isFinite(len)) return;
    const mesh = new THREE.Mesh(cyl, bondMat);
    mesh.position.copy(p0).addScaledVector(dir, 0.5);
    mesh.scale.set(0.085, len, 0.085);
    mesh.quaternion.setFromUnitVectors(up, dir.clone().normalize());
    group.add(mesh);
  };

  // 同一个镜像位置可能被多根键共用，去重后能少画不少球
  const ghostSeen = new Set<string>();
  const addGhost = (atomIndex: number, fracPos: Vec3): void => {
    const key = `${atomIndex}:${fracPos[0].toFixed(4)}:${fracPos[1].toFixed(4)}:${fracPos[2].toFixed(4)}`;
    if (ghostSeen.has(key)) return;
    ghostSeen.add(key);

    const atom = data.atoms[atomIndex];
    const style = elementStyle(atom.element);
    const mesh = new THREE.Mesh(sphere, matFor(atom.element));
    mesh.scale.setScalar(Math.max(style.radius * atomScale, 0.06));
    mesh.position.copy(toCart(fracPos));
    group.add(mesh);
  };

  for (const bond of data.bonds) {
    const fa = frac[bond.a];
    const fb = frac[bond.b];
    const { offset } = minImageOffset(fa, fb, data.cell);
    const crossing = offset[0] !== 0 || offset[1] !== 0 || offset[2] !== 0;

    if (!crossing) {
      addStick(atomPositions[bond.a], atomPositions[bond.b]);
      continue;
    }

    const bGhost: Vec3 = [fb[0] + offset[0], fb[1] + offset[1], fb[2] + offset[2]];
    addStick(atomPositions[bond.a], toCart(bGhost));
    addGhost(bond.b, bGhost);

    const aGhost: Vec3 = [fa[0] - offset[0], fa[1] - offset[1], fa[2] - offset[2]];
    addStick(atomPositions[bond.b], toCart(aGhost));
    addGhost(bond.a, aGhost);
  }

  return group;
}
