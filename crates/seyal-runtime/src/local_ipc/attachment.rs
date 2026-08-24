//! SPEC-004 section 5.2 attachment/authority registry.
//!
//! Every attachment is bound to exactly one authenticated control connection.
//! An `AttachmentId` is an opaque identity, not a bearer capability: another
//! connection cannot use it to mutate, resync, or detach the owner’s session.

use std::collections::HashMap;

use crate::{AttachmentId, ExecutionId, ProjectionId, local_ipc::framing::Role};

pub const MAX_LIVE_ATTACHMENTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentError {
    CapacityExceeded,
    ControllerBusy,
    UnknownAttachment,
    StaleIdentity,
    PermissionDenied,
    WrongConnection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AttachmentRecord {
    execution_id: ExecutionId,
    role: Role,
    projection_id: ProjectionId,
    connection_token: u64,
}

#[derive(Default)]
pub struct AttachmentRegistry {
    attachments: HashMap<AttachmentId, AttachmentRecord>,
    controllers: HashMap<ExecutionId, AttachmentId>,
    max_attachments: usize,
}

impl AttachmentRegistry {
    pub fn new() -> Self {
        Self::with_capacity(MAX_LIVE_ATTACHMENTS)
    }

    pub fn with_capacity(max_attachments: usize) -> Self {
        Self {
            attachments: HashMap::new(),
            controllers: HashMap::new(),
            max_attachments,
        }
    }

    pub fn len(&self) -> usize {
        self.attachments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.attachments.is_empty()
    }

    pub fn insert_prevalidated(
        &mut self,
        attachment_id: AttachmentId,
        execution_id: ExecutionId,
        role: Role,
        projection_id: ProjectionId,
        connection_token: u64,
    ) {
        if role == Role::Controller {
            self.controllers.insert(execution_id, attachment_id);
        }
        self.attachments.insert(
            attachment_id,
            AttachmentRecord {
                execution_id,
                role,
                projection_id,
                connection_token,
            },
        );
    }

    pub fn create_attachment(
        &mut self,
        execution_id: ExecutionId,
        requested_role: Role,
        projection_id: ProjectionId,
        connection_token: u64,
    ) -> Result<AttachmentId, AttachmentError> {
        if requested_role == Role::Controller && self.controllers.contains_key(&execution_id) {
            return Err(AttachmentError::ControllerBusy);
        }
        if self.attachments.len() >= self.max_attachments {
            return Err(AttachmentError::CapacityExceeded);
        }
        let attachment_id = AttachmentId::new();
        self.insert_prevalidated(
            attachment_id,
            execution_id,
            requested_role,
            projection_id,
            connection_token,
        );
        Ok(attachment_id)
    }

    pub fn detach(&mut self, attachment_id: AttachmentId) -> Result<(), AttachmentError> {
        let record = self
            .attachments
            .remove(&attachment_id)
            .ok_or(AttachmentError::UnknownAttachment)?;
        if record.role == Role::Controller
            && self.controllers.get(&record.execution_id) == Some(&attachment_id)
        {
            self.controllers.remove(&record.execution_id);
        }
        Ok(())
    }

    pub fn detach_for_connection(
        &mut self,
        connection_token: u64,
        attachment_id: AttachmentId,
    ) -> Result<(), AttachmentError> {
        self.require_connection(connection_token, attachment_id)?;
        self.detach(attachment_id)
    }

    pub fn attachments_with_connections_for_execution(
        &self,
        execution_id: ExecutionId,
    ) -> Vec<(AttachmentId, u64)> {
        self.attachments
            .iter()
            .filter(|(_, record)| record.execution_id == execution_id)
            .map(|(id, record)| (*id, record.connection_token))
            .collect()
    }

    pub fn remove_all_for_execution(&mut self, execution_id: ExecutionId) -> Vec<AttachmentId> {
        let ids: Vec<AttachmentId> = self
            .attachments
            .iter()
            .filter(|(_, record)| record.execution_id == execution_id)
            .map(|(id, _)| *id)
            .collect();
        for id in &ids {
            self.attachments.remove(id);
        }
        self.controllers.remove(&execution_id);
        ids
    }

    pub fn execution_of(
        &self,
        attachment_id: AttachmentId,
    ) -> Result<ExecutionId, AttachmentError> {
        self.attachments
            .get(&attachment_id)
            .map(|record| record.execution_id)
            .ok_or(AttachmentError::StaleIdentity)
    }

    pub fn execution_for_connection(
        &self,
        connection_token: u64,
        attachment_id: AttachmentId,
    ) -> Result<ExecutionId, AttachmentError> {
        let record = self.require_connection(connection_token, attachment_id)?;
        Ok(record.execution_id)
    }

    pub fn role_of(&self, attachment_id: AttachmentId) -> Result<Role, AttachmentError> {
        self.attachments
            .get(&attachment_id)
            .map(|record| record.role)
            .ok_or(AttachmentError::StaleIdentity)
    }

    pub fn projection_of(
        &self,
        attachment_id: AttachmentId,
    ) -> Result<ProjectionId, AttachmentError> {
        self.attachments
            .get(&attachment_id)
            .map(|record| record.projection_id)
            .ok_or(AttachmentError::StaleIdentity)
    }

    pub fn is_live(&self, attachment_id: AttachmentId, projection_id: ProjectionId) -> bool {
        self.attachments
            .get(&attachment_id)
            .is_some_and(|record| record.projection_id == projection_id)
    }

    pub fn replace_projection(
        &mut self,
        attachment_id: AttachmentId,
        new_projection_id: ProjectionId,
    ) -> Result<(), AttachmentError> {
        self.attachments
            .get_mut(&attachment_id)
            .map(|record| record.projection_id = new_projection_id)
            .ok_or(AttachmentError::StaleIdentity)
    }

    pub fn authorize_mutation(
        &self,
        connection_token: u64,
        attachment_id: AttachmentId,
    ) -> Result<ExecutionId, AttachmentError> {
        let record = self.require_connection(connection_token, attachment_id)?;
        if record.role != Role::Controller {
            return Err(AttachmentError::PermissionDenied);
        }
        Ok(record.execution_id)
    }

    pub fn has_controller(&self, execution_id: ExecutionId) -> bool {
        self.controllers.contains_key(&execution_id)
    }

    pub fn attachments_for_execution(&self, execution_id: ExecutionId) -> usize {
        self.attachments
            .values()
            .filter(|record| record.execution_id == execution_id)
            .count()
    }

    fn require_connection(
        &self,
        connection_token: u64,
        attachment_id: AttachmentId,
    ) -> Result<&AttachmentRecord, AttachmentError> {
        let record = self
            .attachments
            .get(&attachment_id)
            .ok_or(AttachmentError::StaleIdentity)?;
        if record.connection_token != connection_token {
            return Err(AttachmentError::WrongConnection);
        }
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec(id: u128) -> ExecutionId {
        ExecutionId::from_bytes(id.to_le_bytes())
    }

    fn proj(id: u128) -> ProjectionId {
        ProjectionId::from_bytes(id.to_le_bytes())
    }

    #[test]
    fn observer_attach_succeeds_and_grants_read_only_authorization() {
        let mut registry = AttachmentRegistry::new();
        let attachment = registry
            .create_attachment(exec(1), Role::Observer, proj(10), 7)
            .unwrap();
        assert_eq!(registry.role_of(attachment).unwrap(), Role::Observer);
        assert_eq!(
            registry.authorize_mutation(7, attachment),
            Err(AttachmentError::PermissionDenied)
        );
    }

    #[test]
    fn controller_attach_succeeds_and_grants_mutation_authorization() {
        let mut registry = AttachmentRegistry::new();
        let attachment = registry
            .create_attachment(exec(1), Role::Controller, proj(10), 7)
            .unwrap();
        assert_eq!(registry.authorize_mutation(7, attachment), Ok(exec(1)));
        assert!(registry.has_controller(exec(1)));
    }

    #[test]
    fn another_connection_cannot_use_or_detach_the_controller_attachment() {
        let mut registry = AttachmentRegistry::new();
        let attachment = registry
            .create_attachment(exec(1), Role::Controller, proj(10), 7)
            .unwrap();
        assert_eq!(
            registry.authorize_mutation(8, attachment),
            Err(AttachmentError::WrongConnection)
        );
        assert_eq!(
            registry.detach_for_connection(8, attachment),
            Err(AttachmentError::WrongConnection)
        );
        assert_eq!(registry.authorize_mutation(7, attachment), Ok(exec(1)));
    }

    #[test]
    fn second_controller_request_is_rejected_without_preemption() {
        let mut registry = AttachmentRegistry::new();
        let first = registry
            .create_attachment(exec(1), Role::Controller, proj(10), 7)
            .unwrap();
        let second = registry.create_attachment(exec(1), Role::Controller, proj(11), 8);
        assert_eq!(second, Err(AttachmentError::ControllerBusy));
        assert_eq!(registry.authorize_mutation(7, first), Ok(exec(1)));
    }

    #[test]
    fn multiple_observers_may_coexist_with_one_controller() {
        let mut registry = AttachmentRegistry::new();
        let controller = registry
            .create_attachment(exec(1), Role::Controller, proj(10), 1)
            .unwrap();
        let observer_a = registry
            .create_attachment(exec(1), Role::Observer, proj(11), 2)
            .unwrap();
        let observer_b = registry
            .create_attachment(exec(1), Role::Observer, proj(12), 3)
            .unwrap();
        assert_eq!(registry.attachments_for_execution(exec(1)), 3);
        assert_eq!(registry.role_of(observer_a).unwrap(), Role::Observer);
        assert_eq!(registry.role_of(observer_b).unwrap(), Role::Observer);
        assert_eq!(registry.authorize_mutation(1, controller), Ok(exec(1)));
    }

    #[test]
    fn detach_releases_controller_lease_for_a_later_attach() {
        let mut registry = AttachmentRegistry::new();
        let first = registry
            .create_attachment(exec(1), Role::Controller, proj(10), 1)
            .unwrap();
        registry.detach_for_connection(1, first).unwrap();
        assert!(!registry.has_controller(exec(1)));
        assert!(
            registry
                .create_attachment(exec(1), Role::Controller, proj(11), 2)
                .is_ok()
        );
    }

    #[test]
    fn stale_attachment_operations_are_rejected() {
        let registry = AttachmentRegistry::new();
        let bogus = AttachmentId::from_bytes(999u128.to_le_bytes());
        assert_eq!(registry.role_of(bogus), Err(AttachmentError::StaleIdentity));
        assert_eq!(
            registry.authorize_mutation(1, bogus),
            Err(AttachmentError::StaleIdentity)
        );
    }

    #[test]
    fn is_live_rejects_a_stale_projection_id_after_replacement() {
        let mut registry = AttachmentRegistry::new();
        let attachment = registry
            .create_attachment(exec(1), Role::Observer, proj(10), 1)
            .unwrap();
        assert!(registry.is_live(attachment, proj(10)));
        registry.replace_projection(attachment, proj(20)).unwrap();
        assert!(!registry.is_live(attachment, proj(10)));
        assert!(registry.is_live(attachment, proj(20)));
    }

    #[test]
    fn capacity_exceeded_once_the_hard_maximum_is_reached() {
        let mut registry = AttachmentRegistry::with_capacity(2);
        registry
            .create_attachment(exec(1), Role::Observer, proj(10), 1)
            .unwrap();
        registry
            .create_attachment(exec(1), Role::Observer, proj(11), 2)
            .unwrap();
        assert_eq!(
            registry.create_attachment(exec(1), Role::Observer, proj(12), 3),
            Err(AttachmentError::CapacityExceeded)
        );
    }

    #[test]
    fn attachment_connection_enumeration_survives_missing_projection_state() {
        let mut registry = AttachmentRegistry::new();
        let controller = registry
            .create_attachment(exec(1), Role::Controller, proj(10), 7)
            .unwrap();
        let observer = registry
            .create_attachment(exec(1), Role::Observer, proj(11), 8)
            .unwrap();
        let unrelated = registry
            .create_attachment(exec(2), Role::Observer, proj(20), 9)
            .unwrap();
        let mut found = registry.attachments_with_connections_for_execution(exec(1));
        found.sort_by_key(|(id, _)| id.to_string());
        let mut expected = vec![(controller, 7), (observer, 8)];
        expected.sort_by_key(|(id, _)| id.to_string());
        assert_eq!(found, expected);
        assert_eq!(registry.execution_of(unrelated), Ok(exec(2)));
    }

    #[test]
    fn remove_all_for_execution_clears_every_attachment_and_the_lease() {
        let mut registry = AttachmentRegistry::new();
        let controller = registry
            .create_attachment(exec(1), Role::Controller, proj(10), 1)
            .unwrap();
        let observer = registry
            .create_attachment(exec(1), Role::Observer, proj(11), 2)
            .unwrap();
        let unrelated = registry
            .create_attachment(exec(2), Role::Observer, proj(20), 3)
            .unwrap();
        let mut removed = registry.remove_all_for_execution(exec(1));
        removed.sort_by_key(|id| id.to_string());
        let mut expected = vec![controller, observer];
        expected.sort_by_key(|id| id.to_string());
        assert_eq!(removed, expected);
        assert!(!registry.has_controller(exec(1)));
        assert_eq!(registry.attachments_for_execution(exec(1)), 0);
        assert_eq!(registry.execution_of(unrelated), Ok(exec(2)));
    }

    #[test]
    fn repeated_attach_detach_cycles_return_capacity_to_baseline() {
        let mut registry = AttachmentRegistry::with_capacity(1);
        for _ in 0..1000 {
            let attachment = registry
                .create_attachment(exec(1), Role::Controller, proj(1), 1)
                .unwrap();
            registry.detach_for_connection(1, attachment).unwrap();
        }
        assert!(registry.is_empty());
        assert!(!registry.has_controller(exec(1)));
    }
}
