//! SPEC-004 section 5.2 attachment/authority registry.
//!
//! Owns the mapping from `AttachmentId` to its `ExecutionId`/role/current
//! `ProjectionId`, and the single non-preemptive controller lease per
//! `ExecutionId`. Opening a socket grants no attachment or controller
//! authority by itself; every role is explicit and every stale identity is
//! rejected.

use std::collections::HashMap;

use crate::{AttachmentId, ExecutionId, ProjectionId, local_ipc::framing::Role};

/// M001 hard maximum concurrent live attachments (SPEC-004 section 5.1).
pub const MAX_LIVE_ATTACHMENTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentError {
    CapacityExceeded,
    ControllerBusy,
    UnknownAttachment,
    StaleIdentity,
    PermissionDenied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AttachmentRecord {
    execution_id: ExecutionId,
    role: Role,
    projection_id: ProjectionId,
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

    /// Inserts an attachment whose role/capacity constraints the caller has
    /// already validated (used by the single-threaded Runtime wiring layer
    /// once it has reserved real projection resources for a specific,
    /// pre-allocated `AttachmentId` and needs the registry entry to match).
    pub fn insert_prevalidated(
        &mut self,
        attachment_id: AttachmentId,
        execution_id: ExecutionId,
        role: Role,
        projection_id: ProjectionId,
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
            },
        );
    }

    /// Creates a new attachment. A `Controller` request while another
    /// controller already holds the lease for `execution_id` is rejected
    /// with `ControllerBusy` and creates no attachment (SPEC-004 section
    /// 5.2: no implicit preemption).
    pub fn create_attachment(
        &mut self,
        execution_id: ExecutionId,
        requested_role: Role,
        projection_id: ProjectionId,
    ) -> Result<AttachmentId, AttachmentError> {
        if requested_role == Role::Controller && self.controllers.contains_key(&execution_id) {
            return Err(AttachmentError::ControllerBusy);
        }
        if self.attachments.len() >= self.max_attachments {
            return Err(AttachmentError::CapacityExceeded);
        }
        let attachment_id = AttachmentId::new();
        if requested_role == Role::Controller {
            self.controllers.insert(execution_id, attachment_id);
        }
        self.attachments.insert(
            attachment_id,
            AttachmentRecord {
                execution_id,
                role: requested_role,
                projection_id,
            },
        );
        Ok(attachment_id)
    }

    /// Removes an attachment, releasing its controller lease (if any) so a
    /// later `Attach` request may acquire it (SPEC-004 section 5.2:
    /// disconnect/detach revokes the connection's controller lease before
    /// resources are reclaimed).
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

    /// Removes every attachment for `execution_id` (used on execution
    /// finalization) and returns their ids so the caller can notify/close
    /// those connections.
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

    pub fn execution_of(&self, attachment_id: AttachmentId) -> Result<ExecutionId, AttachmentError> {
        self.attachments
            .get(&attachment_id)
            .map(|record| record.execution_id)
            .ok_or(AttachmentError::StaleIdentity)
    }

    pub fn role_of(&self, attachment_id: AttachmentId) -> Result<Role, AttachmentError> {
        self.attachments
            .get(&attachment_id)
            .map(|record| record.role)
            .ok_or(AttachmentError::StaleIdentity)
    }

    pub fn projection_of(&self, attachment_id: AttachmentId) -> Result<ProjectionId, AttachmentError> {
        self.attachments
            .get(&attachment_id)
            .map(|record| record.projection_id)
            .ok_or(AttachmentError::StaleIdentity)
    }

    /// Rejects any `(AttachmentId, ProjectionId)` pair that does not match
    /// the live projection currently associated with that attachment
    /// (SPEC-004 section 8.2: stale projection identifiers cannot affect a
    /// later attachment even from an old mapped client).
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

    /// Authorizes an `Input`/`Resize` request: only the current controller
    /// attachment for its own execution may mutate; an observer attempt is
    /// `PermissionDenied` and a stale/unknown attachment is `StaleIdentity`
    /// (SPEC-004 section 5.2).
    pub fn authorize_mutation(&self, attachment_id: AttachmentId) -> Result<ExecutionId, AttachmentError> {
        let record = self
            .attachments
            .get(&attachment_id)
            .ok_or(AttachmentError::StaleIdentity)?;
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
            .create_attachment(exec(1), Role::Observer, proj(10))
            .unwrap();
        assert_eq!(registry.role_of(attachment).unwrap(), Role::Observer);
        assert_eq!(
            registry.authorize_mutation(attachment),
            Err(AttachmentError::PermissionDenied)
        );
    }

    #[test]
    fn controller_attach_succeeds_and_grants_mutation_authorization() {
        let mut registry = AttachmentRegistry::new();
        let attachment = registry
            .create_attachment(exec(1), Role::Controller, proj(10))
            .unwrap();
        assert_eq!(registry.authorize_mutation(attachment), Ok(exec(1)));
        assert!(registry.has_controller(exec(1)));
    }

    #[test]
    fn second_controller_request_is_rejected_without_preemption() {
        let mut registry = AttachmentRegistry::new();
        let first = registry
            .create_attachment(exec(1), Role::Controller, proj(10))
            .unwrap();
        let second = registry.create_attachment(exec(1), Role::Controller, proj(11));
        assert_eq!(second, Err(AttachmentError::ControllerBusy));
        // The first controller's lease must remain untouched.
        assert_eq!(registry.authorize_mutation(first), Ok(exec(1)));
    }

    #[test]
    fn multiple_observers_may_coexist_with_one_controller() {
        let mut registry = AttachmentRegistry::new();
        let controller = registry
            .create_attachment(exec(1), Role::Controller, proj(10))
            .unwrap();
        let observer_a = registry
            .create_attachment(exec(1), Role::Observer, proj(10))
            .unwrap();
        let observer_b = registry
            .create_attachment(exec(1), Role::Observer, proj(10))
            .unwrap();
        assert_eq!(registry.attachments_for_execution(exec(1)), 3);
        assert!(registry.role_of(observer_a).unwrap() == Role::Observer);
        assert!(registry.role_of(observer_b).unwrap() == Role::Observer);
        assert_eq!(registry.authorize_mutation(controller), Ok(exec(1)));
    }

    #[test]
    fn detach_releases_controller_lease_for_a_later_attach() {
        let mut registry = AttachmentRegistry::new();
        let first = registry
            .create_attachment(exec(1), Role::Controller, proj(10))
            .unwrap();
        registry.detach(first).unwrap();
        assert!(!registry.has_controller(exec(1)));
        let second = registry.create_attachment(exec(1), Role::Controller, proj(11));
        assert!(second.is_ok());
    }

    #[test]
    fn detach_of_unknown_attachment_is_stale_identity() {
        let mut registry = AttachmentRegistry::new();
        let bogus = AttachmentId::from_bytes(999u128.to_le_bytes());
        assert_eq!(registry.detach(bogus), Err(AttachmentError::UnknownAttachment));
    }

    #[test]
    fn stale_attachment_operations_are_rejected() {
        let registry = AttachmentRegistry::new();
        let bogus = AttachmentId::from_bytes(999u128.to_le_bytes());
        assert_eq!(registry.role_of(bogus), Err(AttachmentError::StaleIdentity));
        assert_eq!(
            registry.authorize_mutation(bogus),
            Err(AttachmentError::StaleIdentity)
        );
    }

    #[test]
    fn is_live_rejects_a_stale_projection_id_after_replacement() {
        let mut registry = AttachmentRegistry::new();
        let attachment = registry
            .create_attachment(exec(1), Role::Observer, proj(10))
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
            .create_attachment(exec(1), Role::Observer, proj(10))
            .unwrap();
        registry
            .create_attachment(exec(1), Role::Observer, proj(11))
            .unwrap();
        let third = registry.create_attachment(exec(1), Role::Observer, proj(12));
        assert_eq!(third, Err(AttachmentError::CapacityExceeded));
    }

    #[test]
    fn remove_all_for_execution_clears_every_attachment_and_the_lease() {
        let mut registry = AttachmentRegistry::new();
        let controller = registry
            .create_attachment(exec(1), Role::Controller, proj(10))
            .unwrap();
        let observer = registry
            .create_attachment(exec(1), Role::Observer, proj(10))
            .unwrap();
        let unrelated = registry
            .create_attachment(exec(2), Role::Observer, proj(20))
            .unwrap();

        let mut removed = registry.remove_all_for_execution(exec(1));
        removed.sort_by_key(|id| id.to_string());
        let mut expected = vec![controller, observer];
        expected.sort_by_key(|id| id.to_string());
        assert_eq!(removed, expected);
        assert!(!registry.has_controller(exec(1)));
        assert_eq!(registry.attachments_for_execution(exec(1)), 0);
        // The unrelated execution's attachment must be untouched.
        assert_eq!(registry.execution_of(unrelated), Ok(exec(2)));
    }

    #[test]
    fn repeated_attach_detach_cycles_return_capacity_to_baseline() {
        let mut registry = AttachmentRegistry::with_capacity(1);
        for _ in 0..1000 {
            let attachment = registry
                .create_attachment(exec(1), Role::Controller, proj(1))
                .unwrap();
            registry.detach(attachment).unwrap();
        }
        assert!(registry.is_empty());
        assert!(!registry.has_controller(exec(1)));
    }
}
