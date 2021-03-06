use std::sync::{Arc, RwLock};

use log::debug;
use serde_json::{json, Value};

use crate::behaviour::entity::gate::function::StringGateFunction;
use crate::behaviour::entity::gate::string_gate_properties::StringGateProperties;
use crate::frp::Stream;
use crate::model::{PropertyInstanceGetter, PropertyInstanceSetter, ReactiveEntityInstance};
use crate::reactive::entity::expression::{Expression, ExpressionValue, OperatorPosition};
use crate::reactive::entity::gate::Gate;
use crate::reactive::entity::operation::Operation;
use crate::reactive::entity::Disconnectable;

pub type StringExpressionValue = ExpressionValue<String>;

/// Generic implementation of string_gates operations with two inputs (LHS,RHS) and one result.
///
/// The implementation is realized using reactive streams.
pub struct StringGate<'a> {
    pub lhs: RwLock<Stream<'a, StringExpressionValue>>,

    pub rhs: RwLock<Stream<'a, StringExpressionValue>>,

    pub f: StringGateFunction,

    pub internal_result: RwLock<Stream<'a, String>>,

    pub entity: Arc<ReactiveEntityInstance>,

    pub handle_id: u128,
}

impl StringGate<'_> {
    pub fn new(e: Arc<ReactiveEntityInstance>, f: StringGateFunction) -> StringGate<'static> {
        let lhs = e
            .properties
            .get(StringGateProperties::LHS.as_ref())
            .unwrap()
            .stream
            .read()
            .unwrap()
            .map(|v| match v.as_str() {
                Some(lhs_str) => (OperatorPosition::LHS, String::from(lhs_str)),
                None => (OperatorPosition::LHS, StringGateProperties::LHS.default_value()),
            });
        let rhs = e
            .properties
            .get(StringGateProperties::RHS.as_ref())
            .unwrap()
            .stream
            .read()
            .unwrap()
            .map(|v| -> StringExpressionValue {
                match v.as_str() {
                    Some(rhs_str) => (OperatorPosition::RHS, String::from(rhs_str)),
                    None => (OperatorPosition::RHS, StringGateProperties::RHS.default_value()),
                }
            });

        let expression = lhs.merge(&rhs).fold(
            Expression::new(StringGateProperties::LHS.default_value(), StringGateProperties::RHS.default_value()),
            |old_state, (o, value)| match *o {
                OperatorPosition::LHS => old_state.lhs(String::from(value.clone())),
                OperatorPosition::RHS => old_state.rhs(String::from(value.clone())),
            },
        );

        // The internal result
        let internal_result = expression.map(move |e| f(e.lhs.clone(), e.rhs.clone()));

        let handle_id = e.properties.get(StringGateProperties::RESULT.as_ref()).unwrap().id.as_u128();

        let string_gate = StringGate {
            lhs: RwLock::new(lhs),
            rhs: RwLock::new(rhs),
            f,
            internal_result: RwLock::new(internal_result),
            entity: e.clone(),
            handle_id,
        };

        // Connect the internal result with the stream of the result property
        string_gate.internal_result.read().unwrap().observe_with_handle(
            move |v| {
                debug!("Setting result of string gate: {}", v);
                e.set(StringGateProperties::RESULT.to_string(), json!(*v));
            },
            handle_id,
        );

        string_gate
    }

    /// TODO: extract to trait "Named"
    /// TODO: unit test
    pub fn type_name(&self) -> String {
        self.entity.type_name.clone()
    }
}

impl Disconnectable for StringGate<'_> {
    /// TODO: Add guard: disconnect only if actually connected
    fn disconnect(&self) {
        debug!("Disconnect string gate {} {}", self.type_name(), self.handle_id);
        self.internal_result.read().unwrap().remove(self.handle_id);
    }
}

impl Operation for StringGate<'_> {
    fn lhs(&self, value: Value) {
        self.entity.set(StringGateProperties::LHS.as_ref(), value);
    }

    fn result(&self) -> Value {
        self.entity.get(StringGateProperties::RESULT.as_ref()).unwrap()
    }
}

impl Gate for StringGate<'_> {
    fn rhs(&self, value: Value) {
        self.entity.set(StringGateProperties::RHS.as_ref(), value);
    }
}

/// Automatically disconnect streams on destruction
impl Drop for StringGate<'_> {
    fn drop(&mut self) {
        debug!("Drop string gate");
        self.disconnect();
    }
}
