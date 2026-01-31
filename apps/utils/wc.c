/**
 * wc - 统计文件的行数、单词数和字节数
 * 用于演示操作系统内核的文件读取能力
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <ctype.h>

#define BUFFER_SIZE 4096

typedef struct {
    long lines;
    long words;
    long bytes;
} Counts;

void wc_file(const char *filename, Counts *counts, int show_name) {
    int fd;
    char buffer[BUFFER_SIZE];
    ssize_t bytes_read;
    int in_word = 0;
    
    counts->lines = 0;
    counts->words = 0;
    counts->bytes = 0;
    
    if (strcmp(filename, "-") == 0) {
        fd = STDIN_FILENO;
    } else {
        fd = open(filename, O_RDONLY);
        if (fd < 0) {
            fprintf(stderr, "wc: %s: No such file or directory\n", filename);
            return;
        }
    }
    
    while ((bytes_read = read(fd, buffer, BUFFER_SIZE)) > 0) {
        counts->bytes += bytes_read;
        
        for (ssize_t i = 0; i < bytes_read; i++) {
            char c = buffer[i];
            
            if (c == '\n') {
                counts->lines++;
            }
            
            if (isspace((unsigned char)c)) {
                in_word = 0;
            } else if (!in_word) {
                in_word = 1;
                counts->words++;
            }
        }
    }
    
    if (fd != STDIN_FILENO) {
        close(fd);
    }
    
    printf(" %7ld %7ld %7ld", counts->lines, counts->words, counts->bytes);
    if (show_name) {
        printf(" %s", filename);
    }
    printf("\n");
}

int main(int argc, char *argv[]) {
    Counts total = {0, 0, 0};
    
    if (argc < 2) {
        Counts counts;
        wc_file("-", &counts, 0);
    } else {
        for (int i = 1; i < argc; i++) {
            Counts counts;
            wc_file(argv[i], &counts, 1);
            total.lines += counts.lines;
            total.words += counts.words;
            total.bytes += counts.bytes;
        }
        
        if (argc > 2) {
            printf(" %7ld %7ld %7ld total\n", total.lines, total.words, total.bytes);
        }
    }
    
    return 0;
}
