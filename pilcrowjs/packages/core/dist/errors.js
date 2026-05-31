export class AppError extends Error {
    type;
    status;
    constructor(type, message, status) {
        super(message);
        this.type = type;
        this.status = status;
        this.name = 'AppError';
        Object.setPrototypeOf(this, new.target.prototype);
    }
    static notFound(message = 'Not Found') {
        return new AppError('NotFound', message, 404);
    }
    static unauthorized(message = 'Unauthorized') {
        return new AppError('Unauthorized', message, 401);
    }
    static validation(message) {
        return new AppError('Validation', message, 422);
    }
    static internal(message = 'Internal Server Error') {
        return new AppError('Internal', message, 500);
    }
    static redirect(path) {
        return new AppError('Redirect', path, 303);
    }
}
export const success = (data) => ({ ok: true, data });
export const failure = (error) => ({ ok: false, error });
export class StartupError extends Error {
    code;
    constructor(code, message) {
        super(message);
        this.code = code;
        this.name = 'StartupError';
        Object.setPrototypeOf(this, new.target.prototype);
    }
}
//# sourceMappingURL=errors.js.map