#ifndef SEYAL_BRIDGE_H
#define SEYAL_BRIDGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct SeyalPreparedCell {
    uint32_t scalar;
    uint32_t foreground;
    uint32_t background;
    uint16_t flags;
    uint16_t reserved;
} SeyalPreparedCell;

typedef struct SeyalPreparedFrame {
    const SeyalPreparedCell *cells;
    uint32_t cell_count;
    uint64_t generation;
    uint16_t rows;
    uint16_t columns;
    uint16_t cursor_row;
    uint16_t cursor_column;
    uint8_t cursor_visible;
    uint8_t alternate_screen;
    uint8_t full_rebuild;
    uint8_t reserved0;
    uint16_t rebuilt_row_count;
    uint16_t reserved1;
    uint64_t damage_word0;
    uint64_t damage_word1;
    uint64_t damage_word2;
    uint64_t damage_word3;
} SeyalPreparedFrame;

typedef struct SeyalBlockRecord {
    uint64_t id;
    uint64_t start_line;
    uint64_t end_line;
    uint8_t state;
    uint8_t reserved[3];
    int32_t exit_status;
    const uint8_t *command;
    uint32_t command_len;
} SeyalBlockRecord;

typedef struct SeyalHistoryRow {
    uint64_t line_id;
    const struct SeyalHistoryCell *cells;
    uint32_t cell_count;
} SeyalHistoryRow;

typedef struct SeyalHistoryCell {
    uint32_t scalar;
    uint32_t foreground;
    uint32_t background;
    uint16_t flags;
    uint16_t reserved;
} SeyalHistoryCell;

typedef struct SeyalHistoryRange {
    uint64_t start_line;
    uint64_t end_line;
    uint64_t block_id;
    uint64_t request_id;
    uint64_t revision;
    uint32_t row_count;
    uint32_t reserved;
} SeyalHistoryRange;

typedef struct SeyalComposerResult {
    uint64_t request_id;
    uint64_t block_id;
    uint8_t code;
    uint8_t reserved[7];
} SeyalComposerResult;

enum SeyalTerminalKeyKind {
    SEYAL_KEY_ENTER = 1,
    SEYAL_KEY_TAB = 2,
    SEYAL_KEY_BACKSPACE = 3,
    SEYAL_KEY_ESCAPE = 4,
    SEYAL_KEY_ARROW_UP = 5,
    SEYAL_KEY_ARROW_DOWN = 6,
    SEYAL_KEY_ARROW_RIGHT = 7,
    SEYAL_KEY_ARROW_LEFT = 8,
    SEYAL_KEY_CONTROL_ASCII = 9,
};

int32_t seyal_bridge_connect_first(void);
uint64_t seyal_bridge_open_first(void);
uint64_t seyal_bridge_open_execution(uint64_t execution_low, uint64_t execution_high);
int32_t seyal_bridge_select(uint64_t handle);
void seyal_bridge_disconnect_handle(uint64_t handle);
int32_t seyal_bridge_socket_fd(void);
uint64_t seyal_bridge_execution_id_low(void);
uint64_t seyal_bridge_execution_id_high(void);
int32_t seyal_bridge_poll(void);
int32_t seyal_bridge_wants_write(void);
int32_t seyal_bridge_flush_writable(void);
int32_t seyal_bridge_submit_utf8(const uint8_t *bytes, uint32_t len);
int32_t seyal_bridge_submit_composer(const uint8_t *bytes, uint32_t len);
int32_t seyal_bridge_request_history_range(
    uint64_t block_id,
    uint64_t start_line,
    uint64_t end_line,
    uint16_t max_lines,
    uint32_t max_cells
);
uint64_t seyal_bridge_next_history_request_id(void);
SeyalHistoryRange seyal_bridge_history_range_peek_for(uint64_t block_id, uint64_t request_id);
SeyalHistoryRow seyal_bridge_history_range_row_for(uint64_t block_id, uint64_t request_id, uint32_t index);
uint8_t seyal_bridge_history_range_consume(uint64_t block_id, uint64_t request_id);
SeyalComposerResult seyal_bridge_composer_result(void);
int32_t seyal_bridge_submit_key(uint16_t kind, uint32_t scalar);
int32_t seyal_bridge_propose_geometry(
    double viewport_width,
    double viewport_height,
    double horizontal_insets,
    double vertical_insets,
    double cell_width,
    double cell_height,
    uint8_t meaningful_layout_epoch
);
int32_t seyal_bridge_retry_resize(void);
int32_t seyal_bridge_input_failure(void);
int32_t seyal_bridge_resize_failure(void);
SeyalPreparedFrame seyal_bridge_frame(void);
uint64_t seyal_bridge_block_timeline_revision(void);
uint64_t seyal_bridge_next_composer_request_id(void);
uint32_t seyal_bridge_block_count(void);
SeyalBlockRecord seyal_bridge_block_record(uint32_t index);
void seyal_bridge_disconnect(void);

#ifdef __cplusplus
}
#endif

#endif
