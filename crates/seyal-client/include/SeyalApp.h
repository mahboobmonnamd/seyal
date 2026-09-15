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
    SEYAL_APP_ACTION_ACK_EFFECT = 5,
    SEYAL_APP_ACTION_BEGIN_RECOVERY = 6,
    SEYAL_APP_ACTION_COMPLETE_RECOVERY = 7,
    SEYAL_APP_ACTION_FIRE_RECOVERY = 8,
    SEYAL_APP_ACTION_ACK_RECOVERY = 9,
    SEYAL_APP_ACTION_SET_COMPOSER_DRAFT = 10,
    SEYAL_APP_ACTION_SUBMIT_COMPOSER = 11,
    SEYAL_APP_ACTION_APPLY_COMPOSER_RESULT = 12,
    SEYAL_APP_ACTION_APPLY_RUNTIME_BLOCKS = 13,
    SEYAL_APP_ACTION_SET_LEFT_PANEL = 14,
    SEYAL_APP_ACTION_SET_INSPECTOR = 15,
    SEYAL_APP_ACTION_SELECT_AGENT = 16,
    SEYAL_APP_ACTION_OPEN_ATTENTION = 17,
    SEYAL_APP_ACTION_REPLACE_CHROME = 18,
    SEYAL_APP_ACTION_SELECT_WORKSPACE = 19,
    SEYAL_APP_ACTION_SELECT_TAB = 20,
    SEYAL_APP_ACTION_FOCUS_PANE = 21,
    SEYAL_APP_ACTION_SET_SHELL_CHROME = 22,
    SEYAL_APP_ACTION_CREATE_TAB = 23,
    SEYAL_APP_ACTION_CLOSE_TAB = 24,
    SEYAL_APP_ACTION_SPLIT_FOCUSED = 25,
    SEYAL_APP_ACTION_CLOSE_PANE = 26,
    SEYAL_APP_ACTION_SET_ATTENTION_POPOVER = 27,
    /* reserved = layout node index; target_execution_lo low 16 bits = ratio_bps */
    SEYAL_APP_ACTION_SET_SPLIT_RATIO = 28,
    /* reserved = open (nonzero) */
    SEYAL_APP_ACTION_SET_PALETTE_OPEN = 29,
    /* payload = query utf8 */
    SEYAL_APP_ACTION_SET_PALETTE_QUERY = 30,
    /* reserved = signed move delta as i32 bit pattern */
    SEYAL_APP_ACTION_PALETTE_MOVE = 31,
    SEYAL_APP_ACTION_PALETTE_RUN = 32,
    SEYAL_APP_ACTION_OPEN_COMPOSER_HISTORY = 33,
    SEYAL_APP_ACTION_SET_COMPOSER_HISTORY_QUERY = 34,
    /* reserved = filtered match index; target_pty_generation = composer epoch */
    SEYAL_APP_ACTION_SELECT_COMPOSER_HISTORY = 35,
    SEYAL_APP_ACTION_DISMISS_COMPOSER_HISTORY = 36,
    /* target_execution_lo/hi = BlockId */
    SEYAL_APP_ACTION_SELECT_INSPECTOR_BLOCK = 37,
    SEYAL_APP_ACTION_CLEAR_INSPECTOR_BLOCK = 38
};

enum SeyalAppEligibility {
    SEYAL_APP_ELIGIBILITY_UNBOUND = 0,
    SEYAL_APP_ELIGIBILITY_FLOW = 1,
    SEYAL_APP_ELIGIBILITY_RAW = 2,
    SEYAL_APP_ELIGIBILITY_TUI = 3
};

/*
 * Recovery actions keep the 120-byte SeyalAppAction record.
 * BEGIN/FIRE: target_pty_generation = host clock milliseconds.
 * COMPLETE: target_execution_lo = episode generation,
 *           reserved = outcome | (launch << 8),
 *           target_attachment_lo = opened handle,
 *           target_pty_generation = host clock milliseconds.
 */
enum SeyalAppRecoveryStage {
    SEYAL_APP_RECOVERY_DISCONNECTED = 0,
    SEYAL_APP_RECOVERY_DISCOVERING = 1,
    SEYAL_APP_RECOVERY_STARTING = 2,
    SEYAL_APP_RECOVERY_WAITING_CONTROLLER = 3,
    SEYAL_APP_RECOVERY_RECONSTRUCTING = 4,
    SEYAL_APP_RECOVERY_RESTORING = 5,
    SEYAL_APP_RECOVERY_USABLE = 6,
    SEYAL_APP_RECOVERY_EXHAUSTED = 7,
    SEYAL_APP_RECOVERY_BLOCKED = 8
};

enum SeyalAppRecoveryOutcome {
    SEYAL_APP_RECOVERY_CONNECTED = 0,
    SEYAL_APP_RECOVERY_OPENED_ADOPTED = 1,
    SEYAL_APP_RECOVERY_OPENED_REJECTED = 2,
    SEYAL_APP_RECOVERY_ENDPOINT_MISSING = 3,
    SEYAL_APP_RECOVERY_RETRYABLE = 4,
    SEYAL_APP_RECOVERY_CONTROLLER_BUSY = 5,
    SEYAL_APP_RECOVERY_BLOCKED_OUTCOME = 6
};

enum SeyalAppRecoveryLaunch {
    SEYAL_APP_RECOVERY_LAUNCH_NONE = 0,
    SEYAL_APP_RECOVERY_LAUNCH_STARTED = 1,
    SEYAL_APP_RECOVERY_LAUNCH_HELPER_MISSING = 2
};

enum SeyalAppRecoveryEffect {
    SEYAL_APP_RECOVERY_EFFECT_NONE = 0,
    SEYAL_APP_RECOVERY_EFFECT_PERFORM_ATTEMPT = 1,
    SEYAL_APP_RECOVERY_EFFECT_SCHEDULE = 2,
    SEYAL_APP_RECOVERY_EFFECT_LAUNCH_HELPER = 3,
    SEYAL_APP_RECOVERY_EFFECT_DISPOSE_HANDLE = 4
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

/*
 * Snapshot flags (SeyalAppSnapshot.flags). Distinct from SeyalAppAction.flags.
 * Hosts must translate SNAP_* into FLAG_* when filling an identity fence.
 */
#define SEYAL_APP_SNAP_COMPOSER 1u
#define SEYAL_APP_SNAP_CONTROLLER 2u
#define SEYAL_APP_SNAP_FROZEN 4u
#define SEYAL_APP_SNAP_HAS_EXECUTION 8u
#define SEYAL_APP_SNAP_HAS_ATTACHMENT 16u

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
    uint16_t recovery_stage;
    uint16_t recovery_attempts;
    uint32_t recovery_effect;
    uint64_t recovery_generation;
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

enum SeyalAppComposerMode {
    SEYAL_APP_COMPOSER_HIDDEN = 0,
    SEYAL_APP_COMPOSER_AVAILABLE = 1,
    SEYAL_APP_COMPOSER_BUSY = 2
};

#define SEYAL_APP_COMPOSER_CAN_SUBMIT 1u
#define SEYAL_APP_COMPOSER_DIRECT_TERMINAL 2u
#define SEYAL_APP_COMPOSER_HISTORY_OPEN 4u

#define SEYAL_APP_COPY_COMPOSER_PLACEHOLDER 0u
#define SEYAL_APP_COPY_COMPOSER_EXECUTE 1u
#define SEYAL_APP_COPY_BLOCK_PROMPT 2u
#define SEYAL_APP_COPY_PALETTE_QUERY 3u

#define SEYAL_APP_BLOCK_STATE_RUNNING 1u
#define SEYAL_APP_BLOCK_STATE_COMPLETED 2u
#define SEYAL_APP_BLOCK_STATE_FAILED 3u

typedef struct SeyalAppComposer {
    uint16_t version;
    uint16_t size;
    uint16_t mode;
    uint16_t flags;
    uint64_t epoch;
    uint64_t request_id;
    const uint8_t *draft_utf8;
    uint32_t draft_utf8_len;
    uint32_t block_count;
    uint32_t history_match_count;
    const uint8_t *history_query_utf8;
    uint32_t history_query_utf8_len;
} SeyalAppComposer;

typedef struct SeyalAppChrome {
    uint16_t version;
    uint16_t size;
    uint16_t left_panel;
    uint16_t inspector_mode;
    uint32_t agent_count;
    uint32_t attention_count;
    uint32_t inspector_row_count;
    /* SEYAL_APP_CHROME_* visibility bits. Core Terminal chrome is visible by default. */
    uint32_t reserved;
} SeyalAppChrome;

#define SEYAL_APP_CHROME_LEFT_VISIBLE 1u
#define SEYAL_APP_CHROME_INSPECTOR_VISIBLE 2u
#define SEYAL_APP_CHROME_TAB_STRIP_VISIBLE 4u
#define SEYAL_APP_CHROME_ATTENTION_POPOVER_OPEN 8u
#define SEYAL_APP_CHROME_PALETTE_OPEN 16u

/* SEYAL_APP_ACTION_SPLIT_FOCUSED reserved axis values */
#define SEYAL_APP_SPLIT_RIGHT 0u
#define SEYAL_APP_SPLIT_DOWN 1u

typedef struct SeyalAppAccessibility {
    uint16_t version;
    uint16_t size;
    uint32_t node_count;
    const SeyalAppAxNode *nodes;
    uint32_t reserved;
} SeyalAppAccessibility;

typedef struct SeyalAppTheme {
    uint32_t canvas;
    uint32_t text;
    uint32_t accent;
    uint16_t appearance;
    /* bit0 reduce_transparency, bit1 reduce_motion, bit2 increase_contrast */
    uint16_t flags;
    uint32_t utility_receded;
    uint32_t utility_active;
    uint32_t attention_fill;
    uint32_t overlay;
    uint32_t seam;
    uint16_t intent_receded;   /* 0 opaque, 1 tonal, 2 frosted */
    uint16_t intent_active;
    uint16_t intent_attention;
    uint16_t reserved;
    float focus_duration;
    float overlay_duration;
} SeyalAppTheme;

#define SEYAL_APP_THEME_REDUCE_TRANSPARENCY 1u
#define SEYAL_APP_THEME_REDUCE_MOTION 2u
#define SEYAL_APP_THEME_INCREASE_CONTRAST 4u

#define SEYAL_APP_MATERIAL_OPAQUE 0u
#define SEYAL_APP_MATERIAL_TONAL 1u
#define SEYAL_APP_MATERIAL_FROSTED 2u

#define SEYAL_APP_DEPTH_TRUTH 0u
#define SEYAL_APP_DEPTH_RECEDED 1u
#define SEYAL_APP_DEPTH_ACTIVE 2u
#define SEYAL_APP_DEPTH_ATTENTION 3u

#define SEYAL_APP_SURFACE_LEFT 0u
#define SEYAL_APP_SURFACE_INSPECTOR 1u
#define SEYAL_APP_SURFACE_TAB_STRIP 2u
#define SEYAL_APP_SURFACE_ATTENTION_POPOVER 3u
#define SEYAL_APP_SURFACE_PALETTE_OVERLAY 4u
#define SEYAL_APP_SURFACE_COMPOSER 5u

typedef struct SeyalAppChromeSurface {
    uint16_t surface;
    uint16_t depth;
    uint16_t intent;
    uint16_t reserved;
    uint32_t color;
    uint32_t reserved1;
} SeyalAppChromeSurface;

typedef struct SeyalAppShell {
    uint16_t version;
    uint16_t size;
    uint16_t workspace_count;
    uint16_t tab_count;
    uint16_t pane_count;
    uint16_t flags;
    uint32_t reserved;
    uint64_t active_workspace_lo;
    uint64_t active_workspace_hi;
    uint64_t active_tab_lo;
    uint64_t active_tab_hi;
    uint64_t focused_pane_lo;
    uint64_t focused_pane_hi;
} SeyalAppShell;

#define SEYAL_APP_ROW_WORKSPACE 0u
#define SEYAL_APP_ROW_TAB 1u
#define SEYAL_APP_ROW_PANE 2u
#define SEYAL_APP_ROW_INSPECTOR 0u
#define SEYAL_APP_ROW_AGENT 1u
#define SEYAL_APP_ROW_ATTENTION 2u
#define SEYAL_APP_ROW_PALETTE 3u
#define SEYAL_APP_ROW_SELECTED 1u

#define SEYAL_APP_LAYOUT_LEAF 0u
#define SEYAL_APP_LAYOUT_SPLIT_RIGHT 1u
#define SEYAL_APP_LAYOUT_SPLIT_DOWN 2u

typedef struct SeyalAppLayoutNode {
    uint16_t kind;
    uint16_t flags;
    uint32_t first_child;
    uint32_t second_child;
    /* Split first-child ratio in basis points; 0 on leaves. */
    uint32_t reserved;
    uint64_t pane_lo;
    uint64_t pane_hi;
} SeyalAppLayoutNode;

typedef struct SeyalAppRow {
    uint16_t kind;
    uint16_t flags;
    uint32_t reserved;
    uint64_t id_lo;
    uint64_t id_hi;
    const uint8_t *title;
    uint32_t title_len;
    uint32_t reserved1;
    const uint8_t *detail;
    uint32_t detail_len;
    uint32_t reserved2;
} SeyalAppRow;

uint64_t seyal_app_create(void);
int32_t seyal_app_destroy(uint64_t handle);
int32_t seyal_app_apply(uint64_t handle, const SeyalAppAction *action);
SeyalAppSnapshot seyal_app_snapshot(uint64_t handle);
SeyalAppComposer seyal_app_composer(uint64_t handle);
SeyalAppRow seyal_app_history_row(uint64_t handle, uint32_t index);
SeyalAppChrome seyal_app_chrome(uint64_t handle);
SeyalAppShell seyal_app_shell(uint64_t handle);
SeyalAppRow seyal_app_shell_row(uint64_t handle, uint16_t kind, uint32_t index);
SeyalAppRow seyal_app_chrome_row(uint64_t handle, uint16_t kind, uint32_t index);
SeyalAppRow seyal_app_block_row(uint64_t handle, uint32_t index);
SeyalAppRow seyal_app_copy(uint64_t handle, uint16_t kind);
uint32_t seyal_app_layout_count(uint64_t handle);
SeyalAppLayoutNode seyal_app_layout_node(uint64_t handle, uint32_t index);

typedef struct SeyalAppBlockSpan {
    uint64_t start_line;
    uint64_t end_line;
} SeyalAppBlockSpan;

SeyalAppBlockSpan seyal_app_block_span(uint64_t handle, uint32_t index);
uint64_t seyal_app_recovery_param(uint64_t handle);
SeyalAppAccessibility seyal_app_accessibility(uint64_t handle);
SeyalAppTheme seyal_app_theme(uint16_t appearance, uint16_t accessibility_flags);
SeyalAppChromeSurface seyal_app_chrome_surface(
    uint64_t handle,
    uint16_t surface,
    uint16_t appearance,
    uint16_t accessibility_flags
);
int32_t seyal_app_last_error(uint64_t handle);

#ifdef __cplusplus
}
#endif

#endif
