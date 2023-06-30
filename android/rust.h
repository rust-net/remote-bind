#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

extern "C" {

const char *start(const char *server,
                  uint16_t port,
                  const char *password,
                  const char *local_service);

void stop(const char *handler);

void test();

} // extern "C"
