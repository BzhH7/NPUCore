/**
 * hexdump - 以十六进制格式显示文件内容
 * 用于演示操作系统内核的文件读取能力
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <ctype.h>

#define BYTES_PER_LINE 16

void hexdump_file(const char *filename) {
    int fd;
    unsigned char buffer[BYTES_PER_LINE];
    ssize_t bytes_read;
    unsigned long offset = 0;
    
    if (strcmp(filename, "-") == 0) {
        fd = STDIN_FILENO;
    } else {
        fd = open(filename, O_RDONLY);
        if (fd < 0) {
            fprintf(stderr, "hexdump: %s: No such file or directory\n", filename);
            return;
        }
    }
    
    while ((bytes_read = read(fd, buffer, BYTES_PER_LINE)) > 0) {
        /* 打印偏移量 */
        printf("%08lx  ", offset);
        
        /* 打印十六进制 */
        for (int i = 0; i < BYTES_PER_LINE; i++) {
            if (i == 8) printf(" ");
            if (i < bytes_read) {
                printf("%02x ", buffer[i]);
            } else {
                printf("   ");
            }
        }
        
        /* 打印ASCII */
        printf(" |");
        for (int i = 0; i < bytes_read; i++) {
            if (isprint(buffer[i])) {
                printf("%c", buffer[i]);
            } else {
                printf(".");
            }
        }
        printf("|\n");
        
        offset += bytes_read;
    }
    
    /* 打印最终偏移量 */
    printf("%08lx\n", offset);
    
    if (fd != STDIN_FILENO) {
        close(fd);
    }
}

int main(int argc, char *argv[]) {
    if (argc < 2) {
        hexdump_file("-");
    } else {
        for (int i = 1; i < argc; i++) {
            if (argc > 2) {
                printf("==> %s <==\n", argv[i]);
            }
            hexdump_file(argv[i]);
        }
    }
    
    return 0;
}
