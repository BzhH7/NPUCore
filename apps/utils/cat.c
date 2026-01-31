/**
 * cat - 连接并显示文件内容
 * 用于演示操作系统内核的文件I/O能力
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>

#define BUFFER_SIZE 4096

void cat_file(const char *filename) {
    int fd;
    char buffer[BUFFER_SIZE];
    ssize_t bytes_read;
    
    if (strcmp(filename, "-") == 0) {
        fd = STDIN_FILENO;
    } else {
        fd = open(filename, O_RDONLY);
        if (fd < 0) {
            fprintf(stderr, "cat: %s: No such file or directory\n", filename);
            return;
        }
    }
    
    while ((bytes_read = read(fd, buffer, BUFFER_SIZE)) > 0) {
        ssize_t written = 0;
        while (written < bytes_read) {
            ssize_t w = write(STDOUT_FILENO, buffer + written, bytes_read - written);
            if (w < 0) {
                perror("write");
                break;
            }
            written += w;
        }
    }
    
    if (fd != STDIN_FILENO) {
        close(fd);
    }
}

void cat_stdin(void) {
    char buffer[BUFFER_SIZE];
    ssize_t bytes_read;
    
    while ((bytes_read = read(STDIN_FILENO, buffer, BUFFER_SIZE)) > 0) {
        write(STDOUT_FILENO, buffer, bytes_read);
    }
}

int main(int argc, char *argv[]) {
    if (argc < 2) {
        /* 没有参数时从标准输入读取 */
        cat_stdin();
    } else {
        for (int i = 1; i < argc; i++) {
            cat_file(argv[i]);
        }
    }
    
    return 0;
}
