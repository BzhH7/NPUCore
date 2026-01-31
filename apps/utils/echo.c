/**
 * echo - 显示文本
 * 用于演示操作系统内核的基本输出能力
 */

#include <stdio.h>
#include <string.h>

int main(int argc, char *argv[]) {
    int newline = 1;
    int start = 1;
    
    /* 处理 -n 选项（不输出换行符） */
    if (argc > 1 && strcmp(argv[1], "-n") == 0) {
        newline = 0;
        start = 2;
    }
    
    for (int i = start; i < argc; i++) {
        if (i > start) {
            printf(" ");
        }
        printf("%s", argv[i]);
    }
    
    if (newline) {
        printf("\n");
    }
    
    return 0;
}
