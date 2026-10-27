/** 错误码：与 `crates/vasp-application/src/error.rs` 的 `AppError::code` 一致。 */
export type AppErrorCode =
  | "VALIDATION"
  | "NOT_FOUND"
  | "CONFLICT"
  | "EXTERNAL"
  | "TIMEOUT"
  | "IO"
  | "NOT_IMPLEMENTED"
  | "FAILED";

export interface ErrorPayload {
  code: AppErrorCode;
  message: string;
}

export class DftworkError extends Error {
  readonly code: AppErrorCode;

  constructor(payload: ErrorPayload) {
    super(payload.message);
    this.name = "DftworkError";
    this.code = payload.code;
  }

  /** 「还没实现」和「真的出错了」要给不同的界面反馈。 */
  get isNotImplemented(): boolean {
    return this.code === "NOT_IMPLEMENTED";
  }
}
