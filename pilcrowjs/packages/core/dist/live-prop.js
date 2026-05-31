export function depToString(key) {
    return `${key.table}:${key.column}=${key.value}`;
}
export class LiveProp {
    value;
    dependsOn;
    patchDebounce;
    deliveryTarget = 'dom';
    constructor(value, dependsOn = [], options) {
        this.value = value;
        this.dependsOn = dependsOn.map((dep) => typeof dep === 'string' ? dep : depToString(dep));
        this.patchDebounce = options?.patchDebounce;
        if (options?.target) {
            this.deliveryTarget = options.target;
        }
    }
    static initial(value) {
        return new LiveProp(value, []);
    }
    debounce(seconds) {
        this.patchDebounce = seconds;
        return this;
    }
    target(target) {
        this.deliveryTarget = target;
        return this;
    }
}
//# sourceMappingURL=live-prop.js.map