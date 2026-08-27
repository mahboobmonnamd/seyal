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

int32_t seyal_bridge_connect_first(void);
int32_t seyal_bridge_socket_fd(void);
int32_t seyal_bridge_poll(void);
SeyalPreparedFrame seyal_bridge_frame(void);
void seyal_bridge_disconnect(void);

#ifdef __cplusplus
}
#endif

#endif
