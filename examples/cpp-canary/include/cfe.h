#pragma once

#include <stdint.h>

typedef struct {
    uint8_t bytes[16];
} CFE_MSG_TelemetryHeader_t;

typedef struct {
    uint8_t bytes[16];
} CFE_MSG_CommandHeader_t;
