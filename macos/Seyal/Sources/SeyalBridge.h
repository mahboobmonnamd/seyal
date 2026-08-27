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
int32_t seyal_bridge_socket_fd(void);
int32_t seyal_bridge_poll(void);
int32_t seyal_bridge_wants_write(void);
int32_t seyal_bridge_flush_writable(void);
int32_t seyal_bridge_submit_utf8(const uint8_t *bytes, uint32_t len);
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
void seyal_bridge_disconnect(void);

#ifdef __cplusplus
}
#endif

#endif
