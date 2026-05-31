export interface DependencyKey {
    table: string;
    column: string;
    value: string;
}
export declare function depToString(key: DependencyKey): string;
export type LiveTarget = 'dom' | 'dom-and-store' | 'store';
export declare class LiveProp<T> {
    value: T;
    dependsOn: string[];
    patchDebounce?: number;
    deliveryTarget: LiveTarget;
    constructor(value: T, dependsOn?: (string | DependencyKey)[], options?: {
        patchDebounce?: number;
        target?: LiveTarget;
    });
    static initial<T>(value: T): LiveProp<T>;
    debounce(seconds: number): this;
    target(target: LiveTarget): this;
}
//# sourceMappingURL=live-prop.d.ts.map