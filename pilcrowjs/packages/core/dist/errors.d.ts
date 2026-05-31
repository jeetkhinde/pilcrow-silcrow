export declare class AppError extends Error {
    readonly type: 'NotFound' | 'Unauthorized' | 'Validation' | 'Internal' | 'Redirect';
    readonly status: number;
    constructor(type: 'NotFound' | 'Unauthorized' | 'Validation' | 'Internal' | 'Redirect', message: string, status: number);
    static notFound(message?: string): AppError;
    static unauthorized(message?: string): AppError;
    static validation(message: string): AppError;
    static internal(message?: string): AppError;
    static redirect(path: string): AppError;
}
export type AppResult<T> = {
    ok: true;
    data: T;
} | {
    ok: false;
    error: AppError;
};
export declare const success: <T>(data: T) => AppResult<T>;
export declare const failure: (error: AppError) => AppResult<any>;
export interface HookError {
    status: number;
    message: string;
    source?: string;
}
export declare class StartupError extends Error {
    readonly code: 'ConfigLoad' | 'UnsupportedProvider';
    constructor(code: 'ConfigLoad' | 'UnsupportedProvider', message: string);
}
//# sourceMappingURL=errors.d.ts.map