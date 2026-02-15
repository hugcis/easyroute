#ifndef EASYROUTE_H
#define EASYROUTE_H

#include <stdint.h>

/// Start the EasyRoute server.
/// Returns the actual port (>0) on success, -1 on error.
int32_t easyroute_start(const char *region_path,
                        uint16_t port,
                        const char *mapbox_key,
                        const char *proxy_url);

/// Stop the EasyRoute server gracefully.
void easyroute_stop(void);

#endif /* EASYROUTE_H */
