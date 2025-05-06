#include <stdio.h>
#include <time.h>
#include <unistd.h>

void burn_cpu(int seconds) {
    clock_t start = clock();
    while ((clock() - start) / CLOCKS_PER_SEC < seconds) {
        for (volatile int i = 0; i < 1000000; i++);
    }
}

void light_work() {
    while (1) {
        for (volatile int i = 0; i < 10000; i++) {
            double x = i * 0.0001;
            x = x / (x + 1.0);
        }
        usleep(10000); 
    }
}

int main() {
    printf("Simulating heavy CPU usage for 15 seconds...\n");
    burn_cpu(15);

    printf("Switching to light CPU load...\n");
    light_work();

    return 0;
}
