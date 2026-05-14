use pilcrow_web::live::*;

pub struct Live {
    pub total_count: LiveProp<i64>,
    pub open_count: LiveProp<i64>,
    pub normal_count: LiveProp<i64>,
    pub high_count: LiveProp<i64>,
    pub very_high_count: LiveProp<i64>,
    pub critical_count: LiveProp<i64>,
}

impl Live {
    pub fn query(_params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        live_query!(
            "SELECT \
             COUNT(*)::bigint AS total_count, \
             COUNT(*) FILTER (WHERE status = 'open')::bigint AS open_count, \
             COUNT(*) FILTER (WHERE priority = 'normal')::bigint AS normal_count, \
             COUNT(*) FILTER (WHERE priority = 'high')::bigint AS high_count, \
             COUNT(*) FILTER (WHERE priority = 'very_high')::bigint AS very_high_count, \
             COUNT(*) FILTER (WHERE priority = 'critical')::bigint AS critical_count \
             FROM tickets"
        )
    }
}
