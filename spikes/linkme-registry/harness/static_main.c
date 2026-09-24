// Links libspike_ffi.a statically (iOS simulator).
#include <stddef.h>

size_t spike_report(void);

int main(void) {
    return spike_report() == 5 ? 0 : 1;
}
