//! Predicate evaluation per ADR 0011's "Action evaluation contract".
//!
//! At v0:
//! - `AgentNeed` and `ObjectState` evaluate against the agent's `Needs`
//!   and the smart object's `StateMap` respectively.
//! - `Spatial(_)` always passes (every entity lives in `LeafAreaId::DEFAULT`).
//! - `AgentSkill`, `AgentInventory`, `AgentRelationship`, `MacroState`,
//!   `TimeOfDay` always fail (the systems they depend on don't exist yet,
//!   so any ad referencing them is filtered out).

use crate::agent::{Need, Needs};
use crate::object::{Op, Predicate, StateMap, StateValue};

/// Read-only context for predicate evaluation. Carries everything an
/// `evaluate(...)` call might need from the agent + object.
pub struct EvalContext<'a> {
    pub needs: &'a Needs,
    pub object_state: &'a StateMap,
}

/// Evaluate a single predicate against the agent and (if applicable) the
/// smart-object state. Returns `false` for predicate variants whose
/// referent systems don't exist at v0 (AgentSkill/AgentInventory/
/// AgentRelationship/MacroState/TimeOfDay) — the ad gets filtered out.
#[must_use]
#[allow(dead_code, reason = "called by decide/execute systems in Tasks 4-5")]
pub fn evaluate(predicate: &Predicate, ctx: &EvalContext<'_>) -> bool {
    match predicate {
        Predicate::AgentNeed(need, op, threshold) => {
            apply_op_f32(need_value(ctx.needs, *need), *op, *threshold)
        }
        Predicate::ObjectState(key, op, expected) => ctx
            .object_state
            .get(key)
            .is_some_and(|actual| compare_state_value(actual, *op, expected)),
        Predicate::Spatial(_) => true,
        // v0: missing systems → predicate fails → ad filtered out.
        Predicate::AgentSkill(_, _, _)
        | Predicate::AgentInventory(_, _, _)
        | Predicate::AgentRelationship(_, _, _, _)
        | Predicate::MacroState(_, _, _)
        | Predicate::TimeOfDay(_) => false,
    }
}

fn apply_op_f32(lhs: f32, op: Op, rhs: f32) -> bool {
    match op {
        Op::Lt => lhs < rhs,
        Op::Le => lhs <= rhs,
        Op::Eq => (lhs - rhs).abs() < f32::EPSILON,
        Op::Ge => lhs >= rhs,
        Op::Gt => lhs > rhs,
        Op::Ne => (lhs - rhs).abs() >= f32::EPSILON,
    }
}

fn compare_state_value(actual: &StateValue, op: Op, expected: &StateValue) -> bool {
    match (actual, expected) {
        (StateValue::Bool(a), StateValue::Bool(b)) => match op {
            Op::Eq => a == b,
            Op::Ne => a != b,
            _ => false,
        },
        (StateValue::Int(a), StateValue::Int(b)) => match op {
            Op::Lt => a < b,
            Op::Le => a <= b,
            Op::Eq => a == b,
            Op::Ge => a >= b,
            Op::Gt => a > b,
            Op::Ne => a != b,
        },
        (StateValue::Float(a), StateValue::Float(b)) => apply_op_f32(*a, op, *b),
        (StateValue::Text(a), StateValue::Text(b)) => match op {
            Op::Eq => a == b,
            Op::Ne => a != b,
            _ => false,
        },
        // Type mismatch — predicate fails.
        _ => false,
    }
}

fn need_value(needs: &Needs, need: Need) -> f32 {
    match need {
        Need::Hunger => needs.hunger,
        Need::Sleep => needs.sleep,
        Need::Social => needs.social,
        Need::Hygiene => needs.hygiene,
        Need::Fun => needs.fun,
        Need::Comfort => needs.comfort,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::agent::{Need, Needs};
    use crate::object::{Op, Predicate, SpatialReq, StateValue};
    use crate::systems::decision::predicates::{evaluate, EvalContext};

    fn ctx_with_needs(needs: Needs) -> EvalContext<'static> {
        // Static empty state map for the lifetime of the test fn.
        // We can't construct one inline, so we leak a default for tests.
        // (Tests don't run in tight memory-constrained environments.)
        let leaked: &'static HashMap<String, StateValue> = Box::leak(Box::new(HashMap::new()));
        EvalContext {
            needs: Box::leak(Box::new(needs)),
            object_state: leaked,
        }
    }

    #[test]
    fn agent_need_lt_passes_when_below_threshold() {
        let ctx = ctx_with_needs(Needs {
            hunger: 0.3,
            sleep: 1.0,
            social: 1.0,
            hygiene: 1.0,
            fun: 1.0,
            comfort: 1.0,
        });
        let pred = Predicate::AgentNeed(Need::Hunger, Op::Lt, 0.6);
        assert!(evaluate(&pred, &ctx));
    }

    #[test]
    fn agent_need_lt_fails_when_above_threshold() {
        let ctx = ctx_with_needs(Needs::full());
        let pred = Predicate::AgentNeed(Need::Hunger, Op::Lt, 0.6);
        assert!(!evaluate(&pred, &ctx));
    }

    #[test]
    fn object_state_eq_bool_passes_when_matched() {
        let mut state = HashMap::new();
        state.insert("stocked".to_string(), StateValue::Bool(true));
        let leaked: &'static HashMap<String, StateValue> = Box::leak(Box::new(state));
        let ctx = EvalContext {
            needs: Box::leak(Box::new(Needs::full())),
            object_state: leaked,
        };
        let pred = Predicate::ObjectState("stocked".to_string(), Op::Eq, StateValue::Bool(true));
        assert!(evaluate(&pred, &ctx));
    }

    #[test]
    fn object_state_missing_key_fails() {
        let ctx = ctx_with_needs(Needs::full());
        let pred = Predicate::ObjectState("missing".to_string(), Op::Eq, StateValue::Bool(true));
        assert!(!evaluate(&pred, &ctx));
    }

    #[test]
    fn spatial_always_passes() {
        let ctx = ctx_with_needs(Needs::full());
        for req in [
            SpatialReq::SameLeafArea,
            SpatialReq::AdjacentArea,
            SpatialReq::KnownPlace,
        ] {
            assert!(evaluate(&Predicate::Spatial(req), &ctx));
        }
    }

    #[test]
    fn agent_skill_always_fails_at_v0() {
        let ctx = ctx_with_needs(Needs::full());
        let pred = Predicate::AgentSkill(crate::agent::Skill::Social, Op::Gt, 0.5);
        assert!(!evaluate(&pred, &ctx));
    }
}
