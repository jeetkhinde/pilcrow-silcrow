import { sql } from 'drizzle-orm';
export class FsrStore {
    db;
    globalDebounceSecs;
    redis;
    constructor(db, globalDebounceSecs = 0, redis = null) {
        this.db = db;
        this.globalDebounceSecs = globalDebounceSecs;
        this.redis = redis;
    }
    withGlobalDebounce(secs) {
        this.globalDebounceSecs = secs;
        return this;
    }
    withRedis(redis) {
        this.redis = redis;
        return this;
    }
    async ensureRouteRow(route, promoteAfter) {
        await this.db.execute(sql `
      INSERT INTO pilcrow_fsr (route, slot, promote_after)
      VALUES (${route}, '', ${promoteAfter ?? null})
      ON CONFLICT (route, slot) DO NOTHING
    `);
    }
    async upsertSlot(route, slot, querySql, queryParams, dependsOn, debounceSecs, columnName) {
        await this.db.execute(sql `
      INSERT INTO pilcrow_fsr
        (route, slot, query, query_params, depends_on, debounce_secs, column_name)
      VALUES (
        ${route}, 
        ${slot}, 
        ${querySql}, 
        ${JSON.stringify(queryParams)}, 
        ARRAY(SELECT jsonb_array_elements_text(${JSON.stringify(dependsOn)}::jsonb))::text[], 
        ${debounceSecs ?? null}, 
        ${columnName ?? null}
      )
      ON CONFLICT (route, slot) DO UPDATE SET
        query         = EXCLUDED.query,
        query_params  = EXCLUDED.query_params,
        depends_on    = EXCLUDED.depends_on,
        debounce_secs = EXCLUDED.debounce_secs,
        column_name   = EXCLUDED.column_name
    `);
    }
    async incrementHit(route) {
        const res = await this.db.execute(sql `
      UPDATE pilcrow_fsr
      SET hit_count  = hit_count + 1,
          last_hit   = now(),
          promoted   = CASE
                           WHEN NOT promoted
                                AND promote_after IS NOT NULL
                                AND (hit_count + 1) >= promote_after
                           THEN TRUE
                           ELSE promoted
                       END
      WHERE route = ${route} AND slot = '' AND NOT tombstoned
      RETURNING hit_count as "hitCount", promoted, promote_after as "promoteAfter"
    `);
        const row = res.rows[0];
        if (!row) {
            const checkRes = await this.db.execute(sql `
        SELECT tombstoned FROM pilcrow_fsr WHERE route = ${route} AND slot = '' LIMIT 1
      `);
            const checkRow = checkRes.rows[0];
            if (checkRow && checkRow.tombstoned) {
                return 'Tombstoned';
            }
            return 'Normal';
        }
        const justPromoted = row.promoteAfter !== null &&
            row.promoted &&
            parseInt(row.hitCount, 10) === parseInt(row.promoteAfter, 10);
        return justPromoted ? 'JustPromoted' : 'Normal';
    }
    async tombstone(route) {
        const res = await this.db.execute(sql `
      UPDATE pilcrow_fsr
      SET tombstoned = TRUE, promoted = FALSE, stale = FALSE
      WHERE route = ${route}
      RETURNING slot, html_path as "htmlPath", json_path as "jsonPath"
    `);
        if (this.redis) {
            await this.redis.deleteRouteKeys(route).catch(() => { });
        }
        const rows = res.rows;
        const routeRow = rows.find((r) => r.slot === '');
        if (routeRow) {
            try {
                const fs = await import('fs/promises');
                if (routeRow.htmlPath) {
                    await fs.unlink(routeRow.htmlPath).catch(() => { });
                }
                if (routeRow.jsonPath) {
                    await fs.unlink(routeRow.jsonPath).catch(() => { });
                }
            }
            catch (e) {
                // ignore fs errors
            }
        }
    }
    async isTombstoned(route) {
        const res = await this.db.execute(sql `
      SELECT tombstoned FROM pilcrow_fsr WHERE route = ${route} AND slot = ''
    `);
        const row = res.rows[0];
        return row ? !!row.tombstoned : false;
    }
    async invalidateDepKey(depKey) {
        const res = await this.db.execute(sql `
      UPDATE pilcrow_fsr
      SET stale = TRUE, version = version + 1
      WHERE ${depKey} = ANY(depends_on)
        AND slot != ''
      RETURNING route
    `);
        const routes = Array.from(new Set(res.rows.map((r) => r.route)));
        routes.sort();
        if (this.redis) {
            for (const route of routes) {
                await this.redis.publishInvalidate({
                    route,
                    slots: [],
                    deps: [depKey]
                }).catch(() => { });
            }
        }
        return routes;
    }
    async invalidateRoute(route) {
        await this.db.execute(sql `
      UPDATE pilcrow_fsr
      SET stale = TRUE, version = version + 1
      WHERE route = ${route} AND slot != ''
    `);
        if (this.redis) {
            await this.redis.publishInvalidate({
                route,
                slots: [],
                deps: []
            }).catch(() => { });
        }
    }
    async fetchStaleSlots() {
        const res = await this.db.execute(sql `
      SELECT s.route, s.slot, s.query, s.query_params as "queryParams", s.depends_on as "dependsOn", 
             r.promoted, s.debounce_secs as "debounceSecs", r.html_path as "htmlPath", 
             r.json_path as "jsonPath", s.column_name as "columnName"
      FROM pilcrow_fsr s
      JOIN pilcrow_fsr r ON s.route = r.route AND r.slot = ''
      WHERE s.stale = TRUE AND s.slot != ''
        AND (
          COALESCE(s.debounce_secs, ${this.globalDebounceSecs}) = 0
          OR s.last_patched_at IS NULL
          OR s.last_patched_at + (COALESCE(s.debounce_secs, ${this.globalDebounceSecs}) * interval '1 second') <= NOW()
        )
    `);
        return res.rows.map((r) => ({
            route: r.route,
            slot: r.slot,
            query: r.query,
            queryParams: r.queryParams,
            dependsOn: r.dependsOn || [],
            promoted: !!r.promoted,
            debounceSecs: r.debounceSecs,
            htmlPath: r.htmlPath,
            jsonPath: r.jsonPath,
            columnName: r.columnName
        }));
    }
    async getPromotedPaths(route) {
        const res = await this.db.execute(sql `
      SELECT html_path as "htmlPath", json_path as "jsonPath"
      FROM pilcrow_fsr
      WHERE route = ${route} AND slot = '' AND promoted = TRUE
    `);
        const row = res.rows[0];
        return row ? { htmlPath: row.htmlPath, jsonPath: row.jsonPath } : null;
    }
    async setBakedPaths(route, htmlPath, jsonPath) {
        await this.db.execute(sql `
      UPDATE pilcrow_fsr
      SET html_path = ${htmlPath}, json_path = ${jsonPath}
      WHERE route = ${route} AND slot = ''
    `);
    }
    async evictIdleRoutes(thresholdSecs) {
        const res = await this.db.execute(sql `
      UPDATE pilcrow_fsr
      SET promoted = FALSE, hit_count = 0
      WHERE slot = ''
        AND promoted = TRUE
        AND NOT tombstoned
        AND last_hit < now() - (${thresholdSecs} * interval '1 second')
      RETURNING route, html_path as "htmlPath", json_path as "jsonPath"
    `);
        return res.rows.map((r) => ({
            route: r.route,
            htmlPath: r.htmlPath,
            jsonPath: r.jsonPath
        }));
    }
    async markFresh(route, slot) {
        await this.db.execute(sql `
      UPDATE pilcrow_fsr SET stale = FALSE, version = version + 1, last_patched_at = NOW() WHERE route = ${route} AND slot = ${slot}
    `);
    }
    async fetchSlotsForSnapshot(route, slots) {
        let res;
        if (slots.length === 0) {
            res = await this.db.execute(sql `
        SELECT s.route, s.slot, s.query, s.query_params as "queryParams", s.depends_on as "dependsOn", 
               r.promoted, s.debounce_secs as "debounceSecs", r.html_path as "htmlPath", 
               r.json_path as "jsonPath", s.column_name as "columnName"
        FROM pilcrow_fsr s
        JOIN pilcrow_fsr r ON s.route = r.route AND r.slot = ''
        WHERE s.route = ${route} AND s.slot != ''
        ORDER BY s.slot
      `);
        }
        else {
            res = await this.db.execute(sql `
        SELECT s.route, s.slot, s.query, s.query_params as "queryParams", s.depends_on as "dependsOn", 
               r.promoted, s.debounce_secs as "debounceSecs", r.html_path as "htmlPath", 
               r.json_path as "jsonPath", s.column_name as "columnName"
        FROM pilcrow_fsr s
        JOIN pilcrow_fsr r ON s.route = r.route AND r.slot = ''
        WHERE s.route = ${route} AND s.slot != '' AND s.slot = ANY(ARRAY(SELECT jsonb_array_elements_text(${JSON.stringify(slots)}::jsonb)))
        ORDER BY s.slot
      `);
        }
        return res.rows.map((r) => ({
            route: r.route,
            slot: r.slot,
            query: r.query,
            queryParams: r.queryParams,
            dependsOn: r.dependsOn || [],
            promoted: !!r.promoted,
            debounceSecs: r.debounceSecs,
            htmlPath: r.htmlPath,
            jsonPath: r.jsonPath,
            columnName: r.columnName
        }));
    }
    async fetchAllForInspect() {
        const res = await this.db.execute(sql `
      SELECT route, slot, depends_on as "dependsOn", stale, version, hit_count as "hitCount",
             promoted, html_path as "htmlPath", json_path as "jsonPath",
             to_char(last_hit AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS UTC') AS "lastHit"
      FROM pilcrow_fsr
      ORDER BY route, slot
    `);
        return res.rows.map((r) => ({
            route: r.route,
            slot: r.slot,
            dependsOn: r.dependsOn || [],
            stale: !!r.stale,
            version: r.version,
            hitCount: r.hitCount,
            promoted: !!r.promoted,
            htmlPath: r.htmlPath,
            jsonPath: r.jsonPath,
            lastHit: r.lastHit
        }));
    }
    async reExecuteQuery(slot) {
        if (!slot.query)
            return null;
        const client = this.db.session.client;
        const params = Array.isArray(slot.queryParams) ? slot.queryParams : [];
        const res = await client.query(slot.query, params);
        const row = res.rows[0];
        if (!row)
            return null;
        const colKey = slot.columnName || slot.slot;
        return row[colKey] !== undefined ? row[colKey] : null;
    }
}
//# sourceMappingURL=store.js.map