#ifndef SEYAL_APP_H
#define SEYAL_APP_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Versioned, size-tagged one-Pane application-root ABI (ADR-015 / #906).
 *
 * Pointer-bearing fields are borrowed until the next mutating bridge call
 * (apply, snapshot buffer replace, accessibility, destroy). The host must
 * copy synchronously and may retain only its derived copy. A stale generation
 * must not authorize actions.
 *
 * Product snapshots and Candidate-D prepared frames are separate transfers.
 */

#define SEYAL_APP_ABI_VERSION 1u

enum SeyalAppActionKind {
    SEYAL_APP_ACTION_FOCUS = 0,
    SEYAL_APP_ACTION_BIND = 1,
    SEYAL_APP_ACTION_REFRESH = 2,
    SEYAL_APP_ACTION_SUBMIT_INPUT = 3,
    SEYAL_APP_ACTION_QUIT = 4,
    SEYAL_APP_ACTION_ACK_EFFECT = 5
};

enum SeyalAppEligibility {
    SEYAL_APP_ELIGIBILITY_UNBOUND = 0,
    SEYAL_APP_ELIGIBILITY_FLOW = 1,
    SEYAL_APP_ELIGIBILITY_RAW = 2,
    SEYAL_APP_ELIGIBILITY_TUI = 3
};

enum SeyalAppAxRole {
    SEYAL_APP_AX_APPLICATION = 0,
    SEYAL_APP_AX_PANE = 1,
    SEYAL_APP_AX_COMPOSER = 2,
    SEYAL_APP_AX_TERMINAL = 3
};

typedef struct SeyalAppAction {
    uint16_t version;
    uint16_t size;
    uint16_t kind;
    uint16_t flags;
    uint64_t fence_pane_lo;
    uint64_t fence_pane_hi;
    uint64_t fence_execution_lo;
    uint64_t fence_execution_hi;
    uint64_t fence_attachment_lo;
    uint64_t fence_attachment_hi;
    uint64_t fence_epoch;
    uint64_t target_execution_lo;
    uint64_t target_execution_hi;
    uint64_t target_attachment_lo;
    uint64_t target_attachment_hi;
    uint64_t target_pty_generation;
    const uint8_t *payload;
    uint32_t payload_len;
    uint32_t reserved;
} SeyalAppAction;

#define SEYAL_APP_FLAG_HAS_EXECUTION 1u
#define SEYAL_APP_FLAG_HAS_ATTACHMENT 2u
#define SEYAL_APP_FLAG_CONTROLLER 4u
#define SEYAL_APP_FLAG_ALTERNATE_SCREEN 8u
#define SEYAL_APP_FLAG_TARGET_CONTROLLER 16u

typedef struct SeyalAppSnapshot {
    uint16_t version;
    uint16_t size;
    uint16_t eligibility;
    uint16_t flags;
    uint64_t generation;
    uint64_t pane_lo;
    uint64_t pane_hi;
    uint64_t execution_lo;
    uint64_t execution_hi;
    uint64_t attachment_lo;
    uint64_t attachment_hi;
    uint64_t epoch;
    uint32_t last_error;
    uint32_t pending_effect;
    const uint8_t *output_utf8;
    uint32_t output_utf8_len;
    uint32_t reserved;
} SeyalAppSnapshot;

typedef struct SeyalAppAxNode {
    uint64_t id;
    uint64_t parent;
    uint8_t role;
    uint8_t enabled;
    uint8_t selected;
    uint8_t focused;
    uint32_t actions;
    const uint8_t *label;
    uint32_t label_len;
    uint32_t reserved0;
    const uint8_t *value;
    uint32_t value_len;
    uint32_t reserved1;
    const uint8_t *help;
    uint32_t help_len;
    uint32_t reserved2;
} SeyalAppAxNode;

typedef struct SeyalAppAccessibility {
    uint16_t version;
    uint16_t size;
    uint32_t node_count;
    const SeyalAppAxNode *nodes;
    uint32_t reserved;
} SeyalAppAccessibility;

uint64_t seyal_app_create(void);
int32_t seyal_app_destroy(uint64_t handle);
int32_t seyal_app_apply(uint64_t handle, const SeyalAppAction *action);
SeyalAppSnapshot seyal_app_snapshot(uint64_t handle);
SeyalAppAccessibility seyal_app_accessibility(uint64_t handle);
int32_t seyal_app_last_error(uint64_t handle);

#ifdef __cplusplus
}
#endif

#endif
