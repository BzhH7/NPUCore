/**
 * pwd - 打印当前工作目录
 */

#include <stdio.h>
#include <unistd.h>

#define MAX_PATH 4096

int main(void) {
    char cwd[MAX_PATH];
    
    if (getcwd(cwd, sizeof(cwd)) != NULL) {
        printf("%s\n", cwd);
        return 0;
    } else {
        perror("pwd");
        return 1;
    }
}
