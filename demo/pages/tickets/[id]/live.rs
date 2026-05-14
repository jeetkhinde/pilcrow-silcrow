use pilcrow_web::live::*;
use serde::{Deserialize, Serialize};

/// Unit enum — serde serialises this to a plain JSON string ("open" / "closed").
/// FSR treats it as a scalar: patches [s-live="status"] elements via textContent.
#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    #[default]
    Open,
    Closed,
}

impl std::fmt::Display for TicketStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Closed => write!(f, "closed"),
        }
    }
}

/// Struct — serde serialises this to a JSON object.
/// FSR publishes it to the Silcrow scopeAtom "fsr.priority".
/// Bind in the template with s-use="fsr.priority".
///
/// Field names align with Silcrow's spread-directive keys so the spread
/// updates the element automatically:
///   text  → textContent
///   class → className
///   raw   → inert property (used only for SSR select pre-selection)
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PriorityBadge {
    pub text: String,   // display label: "Normal", "High", "Very High", "Critical"
    pub class: String,  // full CSS class string: "badge badge-critical"
    pub raw: String,    // raw DB value — for {% if live.priority.value.raw == "high" %}
}

#[pilcrow::depends_on_route(tickets, id)]
pub struct Live {
    /// Scalar enum (→ JSON string). FSR patches [s-live="status"] via textContent.
    pub status: LiveProp<TicketStatus>,
    /// Object struct (→ JSON object). FSR publishes to the Silcrow atom "fsr.priority".
    /// Template binds with: s-use="fsr.priority" (no s-live slot needed).
    #[pilcrow::allow_unused]
    pub priority: LiveProp<PriorityBadge>,
}

impl Live {
    pub fn query(params: &serde_json::Map<String, serde_json::Value>) -> LiveQuery {
        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("0");
        live_query!(
            "SELECT
               status,
               json_build_object(
                 'text', CASE priority
                   WHEN 'normal'    THEN 'Normal'
                   WHEN 'high'      THEN 'High'
                   WHEN 'very_high' THEN 'Very High'
                   WHEN 'critical'  THEN 'Critical'
                   ELSE initcap(priority) END,
                 'class', 'badge badge-' || priority,
                 'raw',   priority
               ) AS priority
             FROM tickets WHERE id = $1::bigint",
            id
        )
    }
}
