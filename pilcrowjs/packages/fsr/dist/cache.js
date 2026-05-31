import { Redis } from 'ioredis';
export class RedisCache {
    client;
    artifactTtlSecs = 0;
    constructor(url) {
        this.client = new Redis(url);
    }
    withArtifactTtl(ttlSecs) {
        this.artifactTtlSecs = ttlSecs;
        return this;
    }
    getClient() {
        return this.client;
    }
    htmlKey(route) {
        return `pilcrow:html:${route}`;
    }
    slotKey(route) {
        return `pilcrow:slot:${route}`;
    }
    jsonKey(route) {
        return `pilcrow:json:${route}`;
    }
    async getHtml(route) {
        return this.client.get(this.htmlKey(route));
    }
    async setHtml(route, html) {
        const key = this.htmlKey(route);
        if (this.artifactTtlSecs > 0) {
            await this.client.set(key, html, 'EX', this.artifactTtlSecs);
        }
        else {
            await this.client.set(key, html);
        }
    }
    async patchSlot(route, slot, value) {
        const key = this.slotKey(route);
        if (this.artifactTtlSecs > 0) {
            await this.client.pipeline()
                .hset(key, slot, value)
                .expire(key, this.artifactTtlSecs)
                .exec();
        }
        else {
            await this.client.hset(key, slot, value);
        }
    }
    async getSlots(route) {
        return this.client.hgetall(this.slotKey(route));
    }
    async setJson(route, json) {
        const key = this.jsonKey(route);
        const value = typeof json === 'string' ? json : JSON.stringify(json);
        if (this.artifactTtlSecs > 0) {
            await this.client.set(key, value, 'EX', this.artifactTtlSecs);
        }
        else {
            await this.client.set(key, value);
        }
    }
    async getJson(route) {
        const s = await this.client.get(this.jsonKey(route));
        if (!s)
            return null;
        try {
            return JSON.parse(s);
        }
        catch {
            return null;
        }
    }
    async publishInvalidate(payload) {
        const msg = JSON.stringify(payload);
        await this.client.publish('pilcrow:invalidate', msg);
    }
    async publishPatch(payload) {
        const msg = JSON.stringify(payload);
        await this.client.publish('pilcrow:patch', msg);
    }
    async deleteRouteKeys(route) {
        await this.client.del(this.htmlKey(route), this.slotKey(route), this.jsonKey(route));
    }
    async disconnect() {
        await this.client.quit();
    }
}
//# sourceMappingURL=cache.js.map