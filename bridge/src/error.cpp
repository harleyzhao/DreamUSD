// bridge/src/error.cpp
// Error handling and logging implementation for DreamUSD bridge.
// This file is used in both stub and real USD builds.

#include "dreamusd_bridge.h"
#include "error_internal.h"
#include <string>

static thread_local std::string g_last_error;
static DuLogCallback g_log_callback = nullptr;

void du_set_last_error(const std::string& msg) {
    g_last_error = msg;
}

void du_log(DuLogLevel level, const std::string& msg) {
    if (g_log_callback) {
        g_log_callback(level, msg.c_str());
    }
}

extern "C" {

DuStatus du_get_last_error(const char** message) {
    if (!message) return DU_ERR_NULL;
    *message = g_last_error.c_str();
    return DU_OK;
}

DuStatus du_set_log_callback(DuLogCallback cb) {
    g_log_callback = cb;
    return DU_OK;
}

} // extern "C"
