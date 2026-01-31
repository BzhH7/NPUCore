/**
 * Demo Launcher - OS Kernel Demo Application Launcher
 * Interactive menu to showcase all demo applications
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <termios.h>
#include <fcntl.h>
#include <sys/wait.h>

static struct termios orig_termios;
static int raw_mode = 0;

void disable_raw_mode(void) {
    if (raw_mode) {
        tcsetattr(STDIN_FILENO, TCSAFLUSH, &orig_termios);
        raw_mode = 0;
    }
}

void enable_raw_mode(void) {
    tcgetattr(STDIN_FILENO, &orig_termios);
    struct termios raw = orig_termios;
    raw.c_lflag &= ~(ECHO | ICANON);
    raw.c_cc[VMIN] = 1;
    raw.c_cc[VTIME] = 0;
    tcsetattr(STDIN_FILENO, TCSAFLUSH, &raw);
    raw_mode = 1;
}

void clear_screen(void) {
    printf("\033[2J\033[H");
    fflush(stdout);
}

void print_header(void) {
    printf("\033[36m");  /* Cyan */
    printf("+---------------------------------------------------------------+\n");
    printf("|                                                               |\n");
    printf("|    ___  ____    _  _______ ____  _   _ _____ _                |\n");
    printf("|   / _ \\/ ___|  | |/ / ____|  _ \\| \\ | | ____| |               |\n");
    printf("|  | | | \\___ \\  | ' /|  _| | |_) |  \\| |  _| | |               |\n");
    printf("|  | |_| |___) | | . \\| |___|  _ <| |\\  | |___| |___            |\n");
    printf("|   \\___/|____/  |_|\\_\\_____|_| \\_\\_| \\_|_____|_____|           |\n");
    printf("|                                                               |\n");
    printf("|                 Demo Application Launcher                     |\n");
    printf("|                                                               |\n");
    printf("+---------------------------------------------------------------+\n");
    printf("\033[0m\n");
}

void print_menu(void) {
    printf("  \033[33m[GAMES]\033[0m\n");
    printf("      [1] Tetris     - Classic block puzzle game\n");
    printf("      [2] Snake      - Eat and grow longer\n");
    printf("      [3] 2048       - Merge numbers to win\n");
    printf("\n");
    printf("  \033[33m[APPLICATIONS]\033[0m\n");
    printf("      [4] Kilo       - Minimal text editor\n");
    printf("\n");
    printf("  \033[33m[SYSTEM UTILITIES]\033[0m\n");
    printf("      [5] cat        - Display file contents\n");
    printf("      [6] tree       - Show directory tree\n");
    printf("      [7] cal        - Display calendar\n");
    printf("      [8] hexdump    - Hex file viewer\n");
    printf("\n");
    printf("  \033[33m[BENCHMARKS]\033[0m\n");
    printf("      [9] bench      - Performance tests\n");
    printf("\n");
    printf("  -------------------------------------------\n");
    printf("      [0] Exit       - Quit demo launcher\n");
    printf("\n");
    printf("  \033[32mPress a number key (0-9):\033[0m ");
    fflush(stdout);
}

int run_program(const char *path, char *const argv[]) {
    disable_raw_mode();
    clear_screen();
    
    printf("\033[32m>>> Running: %s\033[0m\n\n", path);
    fflush(stdout);
    
    pid_t pid = fork();
    if (pid == 0) {
        execv(path, argv);
        execvp(argv[0], argv);
        fprintf(stderr, "\033[31mFailed to start: %s\033[0m\n", path);
        _exit(1);
    } else if (pid > 0) {
        int status;
        waitpid(pid, &status, 0);
    }
    
    printf("\n\033[33mPress any key to return to menu...\033[0m");
    fflush(stdout);
    
    enable_raw_mode();
    char c;
    read(STDIN_FILENO, &c, 1);
    
    return 0;
}

int main(void) {
    char c;
    
    enable_raw_mode();
    atexit(disable_raw_mode);
    
    while (1) {
        clear_screen();
        print_header();
        print_menu();
        
        if (read(STDIN_FILENO, &c, 1) != 1) {
            continue;
        }
        
        char *argv_tetris[]  = {"/tetris", NULL};
        char *argv_snake[]   = {"/snake", NULL};
        char *argv_2048[]    = {"/2048", NULL};
        char *argv_kilo[]    = {"/kilo", NULL};
        char *argv_cat[]     = {"/cat", "/etc/passwd", NULL};
        char *argv_tree[]    = {"/tree", "/", NULL};
        char *argv_cal[]     = {"/cal", NULL};
        char *argv_hexdump[] = {"/hexdump", "/demo", NULL};
        char *argv_bench[]   = {"/bench", NULL};
        
        switch (c) {
            case '1':
                run_program("/tetris", argv_tetris);
                break;
            case '2':
                run_program("/snake", argv_snake);
                break;
            case '3':
                run_program("/2048", argv_2048);
                break;
            case '4':
                run_program("/kilo", argv_kilo);
                break;
            case '5':
                run_program("/cat", argv_cat);
                break;
            case '6':
                run_program("/tree", argv_tree);
                break;
            case '7':
                run_program("/cal", argv_cal);
                break;
            case '8':
                run_program("/hexdump", argv_hexdump);
                break;
            case '9':
                run_program("/bench", argv_bench);
                break;
            case '0':
            case 'q':
            case 'Q':
                clear_screen();
                printf("\n  \033[32mThanks for using OS Kernel Demo!\033[0m\n\n");
                return 0;
        }
    }
    
    return 0;
}
