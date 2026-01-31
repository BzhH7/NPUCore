/**
 * Snake Game - 贪吃蛇游戏 (优化版)
 * 使用单缓冲渲染避免闪烁
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <termios.h>
#include <fcntl.h>

/* 游戏区域大小 */
#define WIDTH  30
#define HEIGHT 15
#define MAX_SNAKE_LEN (WIDTH * HEIGHT)

/* 方向定义 */
enum Direction { UP, DOWN, LEFT, RIGHT };

/* 坐标结构 */
typedef struct {
    int x, y;
} Point;

/* 游戏状态 */
typedef struct {
    Point snake[MAX_SNAKE_LEN];
    int snake_len;
    enum Direction dir;
    Point food;
    int score;
    int game_over;
} GameState;

static struct termios orig_termios;
static int orig_flags;

/* 简单随机数 */
static unsigned int seed = 12345;
static int myrand(int max) {
    seed = seed * 1103515245 + 12345;
    return ((seed >> 16) & 0x7FFF) % max;
}

/* 整数转字符串 */
static int itoa_simple(int num, char *buf) {
    char tmp[16];
    int i = 0, j = 0;
    if (num == 0) { buf[0] = '0'; return 1; }
    while (num > 0) { tmp[i++] = '0' + (num % 10); num /= 10; }
    while (i > 0) buf[j++] = tmp[--i];
    return j;
}

/* 恢复终端 */
static void restore_term(void) {
    tcsetattr(STDIN_FILENO, TCSANOW, &orig_termios);
    fcntl(STDIN_FILENO, F_SETFL, orig_flags);
    write(STDOUT_FILENO, "\033[?25h\033[0m", 10);  /* 显示光标,重置颜色 */
}

/* 清屏 */
static void cls(void) {
    write(STDOUT_FILENO, "\033[2J\033[H", 7);
}

/* 生成食物 */
static void spawn_food(GameState *g) {
    int valid;
    do {
        valid = 1;
        g->food.x = myrand(WIDTH - 2) + 1;
        g->food.y = myrand(HEIGHT - 2) + 1;
        for (int i = 0; i < g->snake_len; i++) {
            if (g->snake[i].x == g->food.x && g->snake[i].y == g->food.y) {
                valid = 0;
                break;
            }
        }
    } while (!valid);
}

/* 初始化游戏 */
static void init_game(GameState *g) {
    memset(g, 0, sizeof(GameState));
    g->snake_len = 3;
    g->snake[0].x = WIDTH / 2;
    g->snake[0].y = HEIGHT / 2;
    g->snake[1].x = WIDTH / 2 - 1;
    g->snake[1].y = HEIGHT / 2;
    g->snake[2].x = WIDTH / 2 - 2;
    g->snake[2].y = HEIGHT / 2;
    g->dir = RIGHT;
    g->score = 0;
    g->game_over = 0;
    spawn_food(g);
}

/* 渲染游戏 - 使用单次write避免闪烁 */
static void render(GameState *g) {
    char buf[4096];
    int pos = 0;
    char screen[HEIGHT][WIDTH];
    
    /* 初始化屏幕缓冲 */
    for (int y = 0; y < HEIGHT; y++) {
        for (int x = 0; x < WIDTH; x++) {
            if (y == 0 || y == HEIGHT - 1 || x == 0 || x == WIDTH - 1)
                screen[y][x] = '#';
            else
                screen[y][x] = ' ';
        }
    }
    
    /* 画蛇 */
    for (int i = 0; i < g->snake_len; i++) {
        int x = g->snake[i].x, y = g->snake[i].y;
        if (x >= 0 && x < WIDTH && y >= 0 && y < HEIGHT)
            screen[y][x] = (i == 0) ? '@' : 'o';
    }
    
    /* 画食物 */
    if (g->food.x >= 0 && g->food.x < WIDTH && g->food.y >= 0 && g->food.y < HEIGHT)
        screen[g->food.y][g->food.x] = '*';
    
    /* 构建输出缓冲 - 移动到左上角 */
    buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = 'H';
    
    /* 输出屏幕内容 */
    for (int y = 0; y < HEIGHT; y++) {
        for (int x = 0; x < WIDTH; x++) {
            char c = screen[y][x];
            if (c == '@') {
                /* 绿色蛇头 */
                memcpy(buf + pos, "\033[32m@\033[0m", 10);
                pos += 10;
            } else if (c == 'o') {
                /* 亮绿色蛇身 */
                memcpy(buf + pos, "\033[92mo\033[0m", 10);
                pos += 10;
            } else if (c == '*') {
                /* 红色食物 */
                memcpy(buf + pos, "\033[31m*\033[0m", 10);
                pos += 10;
            } else if (c == '#') {
                /* 青色边框 */
                memcpy(buf + pos, "\033[36m#\033[0m", 10);
                pos += 10;
            } else {
                buf[pos++] = c;
            }
        }
        buf[pos++] = '\n';
    }
    
    /* 分数信息 */
    memcpy(buf + pos, "Score: ", 7); pos += 7;
    pos += itoa_simple(g->score, buf + pos);
    memcpy(buf + pos, "  Length: ", 10); pos += 10;
    pos += itoa_simple(g->snake_len, buf + pos);
    buf[pos++] = '\n';
    memcpy(buf + pos, "WASD: Move  Q: Quit", 19); pos += 19;
    buf[pos++] = '\n';
    
    write(STDOUT_FILENO, buf, pos);
}

/* 更新游戏 */
static void update(GameState *g) {
    Point new_head = g->snake[0];
    
    switch (g->dir) {
        case UP:    new_head.y--; break;
        case DOWN:  new_head.y++; break;
        case LEFT:  new_head.x--; break;
        case RIGHT: new_head.x++; break;
    }
    
    /* 碰撞边界 */
    if (new_head.x <= 0 || new_head.x >= WIDTH - 1 ||
        new_head.y <= 0 || new_head.y >= HEIGHT - 1) {
        g->game_over = 1;
        return;
    }
    
    /* 碰撞自身 */
    for (int i = 0; i < g->snake_len; i++) {
        if (g->snake[i].x == new_head.x && g->snake[i].y == new_head.y) {
            g->game_over = 1;
            return;
        }
    }
    
    /* 吃食物 */
    int ate = (new_head.x == g->food.x && new_head.y == g->food.y);
    
    if (!ate) {
        /* 移动蛇身 */
        for (int i = g->snake_len - 1; i > 0; i--)
            g->snake[i] = g->snake[i - 1];
    } else {
        /* 增长 */
        for (int i = g->snake_len; i > 0; i--)
            g->snake[i] = g->snake[i - 1];
        g->snake_len++;
        g->score += 10;
        spawn_food(g);
    }
    
    g->snake[0] = new_head;
}

/* 简单延时 */
static void delay(int loops) {
    volatile int i;
    for (i = 0; i < loops; i++);
}

int main(void) {
    GameState game;
    char c;
    struct termios raw;
    int frame_delay = 300000;  /* 调整此值控制速度 */
    
    /* 设置终端 */
    tcgetattr(STDIN_FILENO, &orig_termios);
    orig_flags = fcntl(STDIN_FILENO, F_GETFL, 0);
    
    raw = orig_termios;
    raw.c_lflag &= ~(ECHO | ICANON);
    raw.c_cc[VMIN] = 1;
    raw.c_cc[VTIME] = 0;
    tcsetattr(STDIN_FILENO, TCSANOW, &raw);
    
    /* 隐藏光标 */
    write(STDOUT_FILENO, "\033[?25l", 6);
    
    cls();
    write(STDOUT_FILENO, "\n  === SNAKE GAME ===\n\n", 23);
    write(STDOUT_FILENO, "  W/A/S/D - Move\n", 17);
    write(STDOUT_FILENO, "  Q - Quit\n\n", 12);
    write(STDOUT_FILENO, "  Press any key to start...\n", 28);
    
    read(STDIN_FILENO, &c, 1);
    if (c == 'q' || c == 'Q') {
        restore_term();
        return 0;
    }

restart:
    init_game(&game);
    
    /* 设置非阻塞读取 */
    fcntl(STDIN_FILENO, F_SETFL, orig_flags | O_NONBLOCK);
    
    cls();
    
    while (!game.game_over) {
        render(&game);
        
        /* 读取输入 */
        while (read(STDIN_FILENO, &c, 1) == 1) {
            if (c == '\033') {
                char seq[2];
                if (read(STDIN_FILENO, &seq[0], 1) == 1 &&
                    read(STDIN_FILENO, &seq[1], 1) == 1 && seq[0] == '[') {
                    switch (seq[1]) {
                        case 'A': c = 'w'; break;
                        case 'B': c = 's'; break;
                        case 'C': c = 'd'; break;
                        case 'D': c = 'a'; break;
                    }
                }
            }
            switch (c) {
                case 'w': case 'W': if (game.dir != DOWN) game.dir = UP; break;
                case 's': case 'S': if (game.dir != UP) game.dir = DOWN; break;
                case 'a': case 'A': if (game.dir != RIGHT) game.dir = LEFT; break;
                case 'd': case 'D': if (game.dir != LEFT) game.dir = RIGHT; break;
                case 'q': case 'Q': game.game_over = 1; break;
            }
        }
        
        update(&game);
        delay(frame_delay);
    }
    
    /* 恢复阻塞模式 */
    fcntl(STDIN_FILENO, F_SETFL, orig_flags);
    
    cls();
    write(STDOUT_FILENO, "\n  === GAME OVER ===\n\n", 22);
    {
        char msg[64];
        int len = 0;
        memcpy(msg, "  Score: ", 9); len = 9;
        len += itoa_simple(game.score, msg + len);
        msg[len++] = '\n';
        memcpy(msg + len, "  Length: ", 10); len += 10;
        len += itoa_simple(game.snake_len, msg + len);
        msg[len++] = '\n'; msg[len++] = '\n';
        write(STDOUT_FILENO, msg, len);
    }
    write(STDOUT_FILENO, "  R - Restart\n", 14);
    write(STDOUT_FILENO, "  Q - Quit\n", 11);
    
    while (1) {
        if (read(STDIN_FILENO, &c, 1) == 1) {
            if (c == 'r' || c == 'R') goto restart;
            if (c == 'q' || c == 'Q') break;
        }
    }
    
    restore_term();
    cls();
    return 0;
}
