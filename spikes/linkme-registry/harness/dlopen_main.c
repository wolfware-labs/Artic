// Loads libspike_ffi.so at runtime, the way an Android app loads native code.
#include <dlfcn.h>
#include <stddef.h>
#include <stdio.h>

int main(int argc, char **argv) {
    const char *path = argc > 1 ? argv[1] : "./libspike_ffi.so";
    void *lib = dlopen(path, RTLD_NOW);
    if (!lib) {
        printf("dlopen failed: %s\n", dlerror());
        return 2;
    }
    size_t (*report)(void) = (size_t (*)(void))dlsym(lib, "spike_report");
    if (!report) {
        printf("dlsym failed: %s\n", dlerror());
        return 2;
    }
    return report() == 5 ? 0 : 1;
}
