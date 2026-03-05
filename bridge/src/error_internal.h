#ifndef DREAMUSD_ERROR_INTERNAL_H
#define DREAMUSD_ERROR_INTERNAL_H

#include "dreamusd_bridge.h"
#include <string>

void du_set_last_error(const std::string& msg);
void du_log(DuLogLevel level, const std::string& msg);

#define DU_TRY(expr) \
    try { expr; } catch (const std::exception& e) { \
        du_set_last_error(e.what()); \
        return DU_ERR_USD; \
    }

#define DU_CHECK_NULL(ptr) \
    if (!(ptr)) { du_set_last_error(#ptr " is null"); return DU_ERR_NULL; }

#endif // DREAMUSD_ERROR_INTERNAL_H
