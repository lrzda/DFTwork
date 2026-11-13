import { z } from "zod";
import { DftworkError } from "@dftwork/shared";

const pathString = z.string().trim().min(1).max(32_768);
const optionalText = z.string().trim().min(1).max(256).optional();
const modelSettings = {
  model: optionalText,
  provider: optionalText,
  maxTurns: z.number().int().min(1).max(50).optional(),
};

const buildItem = z.object({
  rel_path: z.string().min(1),
  content: z.string(),
  action: z.enum(["create", "overwrite", "skip"]),
  reason: z.string(),
  previous_content: z.string().nullable(),
}).strict();

const buildPlan = z.object({
  root: pathString,
  items: z.array(buildItem),
  plan_hash: z.string().min(1),
  created_at: z.string().min(1),
}).strict();

const buildReceipt = z.object({
  root: pathString,
  plan_hash: z.string().min(1),
  written: z.array(z.string()),
  skipped: z.array(z.string()),
  backup_dir: z.string().nullable(),
  restores: z.array(z.object({
    rel_path: z.string().min(1),
    blob: z.string().nullable(),
    applied_hash: z.string().nullable(),
  }).strict()),
  finished_at: z.string().min(1),
}).strict();

export const requests = {
  scan: z.object({
    root: pathString,
    maxDepth: z.number().int().min(1).max(64).optional(),
  }).strict(),
  convergence: z.object({ taskAbsPath: pathString }).strict(),
  scene: z.object({
    fileAbsPath: pathString,
    pbc: z.boolean().optional(),
  }).strict(),
  task: z.object({ taskAbsPath: pathString }).strict(),
  trajectory: z.object({ taskAbsPath: pathString }).strict(),
  compare: z.object({
    taskAbsPaths: z.array(pathString).min(2).max(100),
  }).strict(),
  build: z.object({
    root: pathString,
    sourceRel: z.string().trim().min(1).max(4096),
    targetRel: z.string().trim().min(1).max(4096),
    overrides: z.array(z.tuple([z.string().trim().min(1).max(256), z.string().max(4096)])).max(200),
  }).strict(),
  apply: z.object({ plan: buildPlan }).strict(),
  rollback: z.object({ receipt: buildReceipt }).strict(),
  projectRoot: z.object({ root: pathString }).strict(),
  analyze: z.object({
    taskAbsPath: pathString,
    ...modelSettings,
  }).strict(),
  hermesChat: z.object({
    root: pathString,
    message: z.string().trim().min(1).max(32_000),
    sessionId: z.string().trim().min(1).max(512).optional(),
    ...modelSettings,
  }).strict(),
};

export function parseRequest<T>(schema: z.ZodType<T>, value: unknown): T {
  const result = schema.safeParse(value);
  if (result.success) return result.data;
  const message = result.error.issues
    .map((issue) => `${issue.path.join(".") || "请求"}: ${issue.message}`)
    .join("；");
  throw new DftworkError({ code: "VALIDATION", message });
}
