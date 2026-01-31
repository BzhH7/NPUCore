/**
 * Tetris - 俄罗斯方块 (简化版)
 * 适用于简单内核环境
 */

#include <unistd.h>
#include <termios.h>
#include <string.h>
#include <fcntl.h>
#include <time.h>
#include <sys/time.h>

#define BOARD_W 10
#define BOARD_H 18

/* 7种方块形状 */
static const int SHAPES[7][4][2] = {
    {{0,0}, {1,0}, {2,0}, {3,0}},  /* I */
    {{0,0}, {1,0}, {0,1}, {1,1}},  /* O */
    {{0,0}, {1,0}, {2,0}, {1,1}},  /* T */
    {{1,0}, {2,0}, {0,1}, {1,1}},  /* S */
    {{0,0}, {1,0}, {1,1}, {2,1}},  /* Z */
    {{0,0}, {0,1}, {1,1}, {2,1}},  /* J */
    {{2,0}, {0,1}, {1,1}, {2,1}},  /* L */
};

static int board[BOARD_H][BOARD_W];
static int cur_type, cur_x, cur_y, cur_rot;
static int cur_blocks[4][2];
static int next_type;
static int score, level, lines;
static int game_over;
static struct termios orig_termios;
static int orig_flags;

static unsigned int seed = 54321;
static int myrand(int max) {
    seed = seed * 1103515245 + 12345;
    unsigned int val = (seed / 65536) % 32768;
    return (int)(val % max);
}

static int itoa_simple(int num, char *buf) {
    char tmp[16];
    int i = 0, j = 0;
    if (num == 0) { buf[0] = '0'; return 1; }
    while (num > 0) { tmp[i++] = '0' + (num % 10); num /= 10; }
    while (i > 0) buf[j++] = tmp[--i];
    return j;
}

static void restore_term(void) {
    tcsetattr(STDIN_FILENO, TCSANOW, &orig_termios);
    fcntl(STDIN_FILENO, F_SETFL, orig_flags);
    write(STDOUT_FILENO, "\033[?25h\033[0m\n", 11);
}

static void cls(void) {
    write(STDOUT_FILENO, "\033[2J\033[H", 7);
}

/* 计算旋转后的方块位置 */
static void calc_blocks(int type, int rot, int px, int py, int out[4][2]) {
    for (int i = 0; i < 4; i++) {
        int bx = SHAPES[type][i][0];
        int by = SHAPES[type][i][1];
        int rx, ry;
        
        /* I 型方块特殊处理（4x1 -> 1x4） */
        if (type == 0) {
            switch (rot % 2) {
                case 0: rx = bx; ry = by; break;  /* 横向：0,0 1,0 2,0 3,0 */
                case 1: rx = 1; ry = bx; break;   /* 竖向：1,0 1,1 1,2 1,3 */
                default: rx = bx; ry = by; break;
            }
        } else {
            /* 其他方块使用 3x3 旋转 */
            switch (rot % 4) {
                case 0: rx = bx; ry = by; break;
                case 1: rx = 2 - by; ry = bx; break;
                case 2: rx = 2 - bx; ry = 2 - by; break;
                case 3: rx = by; ry = 2 - bx; break;
                default: rx = bx; ry = by; break;
            }
        }
        out[i][0] = px + rx;
        out[i][1] = py + ry;
    }
}

/* 碰撞检测 */
static int collide(int type, int rot, int px, int py) {
    int blocks[4][2];
    calc_blocks(type, rot, px, py, blocks);
    for (int i = 0; i < 4; i++) {
        int bx = blocks[i][0], by = blocks[i][1];
        if (bx < 0 || bx >= BOARD_W || by >= BOARD_H) return 1;
        if (by >= 0 && board[by][bx]) return 1;
    }
    return 0;
}

/* 生成新方块 */
static void new_piece(void) {
    cur_type = next_type;
    next_type = myrand(7);
    cur_x = BOARD_W / 2 - 1;
    cur_y = -1;
    cur_rot = 0;
    calc_blocks(cur_type, cur_rot, cur_x, cur_y, cur_blocks);
}

/* 锁定方块到棋盘 */
static void lock_piece(void) {
    for (int i = 0; i < 4; i++) {
        int bx = cur_blocks[i][0], by = cur_blocks[i][1];
        if (by >= 0 && by < BOARD_H && bx >= 0 && bx < BOARD_W)
            board[by][bx] = cur_type + 1;
    }
}

/* 消除满行 */
static void clear_lines_func(void) {
    int cleared = 0;
    for (int y = BOARD_H - 1; y >= 0; y--) {
        int full = 1;
        for (int x = 0; x < BOARD_W; x++) {
            if (!board[y][x]) { full = 0; break; }
        }
        if (full) {
            cleared++;
            for (int k = y; k > 0; k--) {
                for (int x = 0; x < BOARD_W; x++)
                    board[k][x] = board[k-1][x];
            }
            for (int x = 0; x < BOARD_W; x++) board[0][x] = 0;
            y++;
        }
    }
    if (cleared > 0) {
        int pts[] = {0, 40, 100, 300, 1200};
        lines += cleared;
        score += pts[cleared] * (level + 1);
        level = lines / 10;
        if (level > 9) level = 9;
    }
}

/* 渲染游戏 */
static void render(void) {
    char buf[2048];
    int pos = 0;
    int disp[BOARD_H][BOARD_W];
    
    memcpy(disp, board, sizeof(disp));
    for (int i = 0; i < 4; i++) {
        int bx = cur_blocks[i][0], by = cur_blocks[i][1];
        if (by >= 0 && by < BOARD_H && bx >= 0 && bx < BOARD_W)
            disp[by][bx] = 8;  /* 当前方块标记为8 */
    }
    
    /* 移动到左上角 */
    buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = 'H';
    
    /* 顶部边框 */
    buf[pos++] = '+';
    for (int x = 0; x < BOARD_W * 2; x++) buf[pos++] = '-';
    buf[pos++] = '+'; buf[pos++] = '\n';
    
    /* 游戏区域 */
    for (int y = 0; y < BOARD_H; y++) {
        buf[pos++] = '|';
        for (int x = 0; x < BOARD_W; x++) {
            if (disp[y][x] == 8) {
                /* 当前方块 - 黄色 */
                memcpy(buf + pos, "\033[33m[]\033[0m", 12);
                pos += 12;
            } else if (disp[y][x]) {
                /* 固定方块 - 青色 */
                memcpy(buf + pos, "\033[36m[]\033[0m", 12);
                pos += 12;
            } else {
                buf[pos++] = ' ';
                buf[pos++] = ' ';
            }
        }
        buf[pos++] = '|';
        
        /* 右侧信息 */
        if (y == 1) {
            memcpy(buf + pos, " Level: ", 8); pos += 8;
            pos += itoa_simple(level + 1, buf + pos);
        } else if (y == 3) {
            memcpy(buf + pos, " Score: ", 8); pos += 8;
            pos += itoa_simple(score, buf + pos);
        } else if (y == 5) {
            memcpy(buf + pos, " Lines: ", 8); pos += 8;
            pos += itoa_simple(lines, buf + pos);
        } else if (y == 8) {
            memcpy(buf + pos, " Controls:", 10); pos += 10;
        } else if (y == 9) {
            memcpy(buf + pos, " A/D Move", 9); pos += 9;
        } else if (y == 10) {
            memcpy(buf + pos, " W Rotate", 9); pos += 9;
        } else if (y == 11) {
            memcpy(buf + pos, " S Drop", 7); pos += 7;
        } else if (y == 12) {
            memcpy(buf + pos, " Q Quit", 7); pos += 7;
        }
        buf[pos++] = '\n';
    }
    
    /* 底部边框 */
    buf[pos++] = '+';
    for (int x = 0; x < BOARD_W * 2; x++) buf[pos++] = '-';
    buf[pos++] = '+'; buf[pos++] = '\n';
    
    write(STDOUT_FILENO, buf, pos);
}

static void init_game(void) {
    memset(board, 0, sizeof(board));
    score = 0;
    level = 0;
    lines = 0;
    game_over = 0;
    next_type = myrand(7);
    new_piece();
}

/* 简单延时 */
static void delay(void) {
    volatile int i;
    for (i = 0; i < 400000; i++);
}

int main(void) {
    char c;
    struct termios raw;
    int drop_counter = 0;
    int drop_interval;
    struct timeval tv;
    
    /* 使用时间初始化随机数种子 */
    gettimeofday(&tv, 0);
    seed = (unsigned int)(tv.tv_sec * 1000000 + tv.tv_usec);
    
    tcgetattr(STDIN_FILENO, &orig_termios);
    orig_flags = fcntl(STDIN_FILENO, F_GETFL, 0);
    
    raw = orig_termios;
    raw.c_lflag &= ~(ECHO | ICANON);
    raw.c_cc[VMIN] = 1;
    raw.c_cc[VTIME] = 0;
    tcsetattr(STDIN_FILENO, TCSANOW, &raw);
    write(STDOUT_FILENO, "\033[?25l", 6);
    
    cls();
    write(STDOUT_FILENO, "\n  === TETRIS ===\n\n", 19);
    write(STDOUT_FILENO, "  W - Rotate\n", 13);
    write(STDOUT_FILENO, "  A/D - Move Left/Right\n", 24);
    write(STDOUT_FILENO, "  S - Hard Drop\n", 16);
    write(STDOUT_FILENO, "  Q - Quit\n\n", 12);
    write(STDOUT_FILENO, "  Press any key to start...\n", 28);
    
    read(STDIN_FILENO, &c, 1);
    if (c == 'q' || c == 'Q') {
        restore_term();
        return 0;
    }

restart:
    init_game();
    drop_interval = 12 - level;
    if (drop_interval < 3) drop_interval = 3;
    
    fcntl(STDIN_FILENO, F_SETFL, orig_flags | O_NONBLOCK);
    cls();
    
    while (!game_over) {
        render();
        
        /* 处理输入 */
        while (read(STDIN_FILENO, &c, 1) == 1) {
            switch (c) {
                case 'a': case 'A':
                    if (!collide(cur_type, cur_rot, cur_x - 1, cur_y)) {
                        cur_x--;
                        calc_blocks(cur_type, cur_rot, cur_x, cur_y, cur_blocks);
                    }
                    break;
                case 'd': case 'D':
                    if (!collide(cur_type, cur_rot, cur_x + 1, cur_y)) {
                        cur_x++;
                        calc_blocks(cur_type, cur_rot, cur_x, cur_y, cur_blocks);
                    }
                    break;
                case 'w': case 'W':
                    if (!collide(cur_type, cur_rot + 1, cur_x, cur_y)) {
                        cur_rot++;
                        calc_blocks(cur_type, cur_rot, cur_x, cur_y, cur_blocks);
                    }
                    break;
                case 's': case 'S':
                    while (!collide(cur_type, cur_rot, cur_x, cur_y + 1))
                        cur_y++;
                    calc_blocks(cur_type, cur_rot, cur_x, cur_y, cur_blocks);
                    lock_piece();
                    clear_lines_func();
                    new_piece();
                    if (collide(cur_type, cur_rot, cur_x, cur_y))
                        game_over = 1;
                    drop_counter = 0;
                    break;
                case 'q': case 'Q':
                    game_over = 1;
                    break;
            }
        }
        
        if (game_over) break;
        
        /* 自动下落 */
        drop_counter++;
        if (drop_counter >= drop_interval) {
            drop_counter = 0;
            if (!collide(cur_type, cur_rot, cur_x, cur_y + 1)) {
                cur_y++;
                calc_blocks(cur_type, cur_rot, cur_x, cur_y, cur_blocks);
            } else {
                lock_piece();
                clear_lines_func();
                new_piece();
                if (collide(cur_type, cur_rot, cur_x, cur_y))
                    game_over = 1;
                drop_interval = 12 - level;
                if (drop_interval < 3) drop_interval = 3;
            }
        }
        
        delay();
    }
    
    fcntl(STDIN_FILENO, F_SETFL, orig_flags);
    
    cls();
    {
        char msg[128];
        int len = 0;
        memcpy(msg, "\n  === GAME OVER ===\n\n", 22); len = 22;
        memcpy(msg + len, "  Final Score: ", 15); len += 15;
        len += itoa_simple(score, msg + len);
        msg[len++] = '\n';
        memcpy(msg + len, "  Lines Cleared: ", 17); len += 17;
        len += itoa_simple(lines, msg + len);
        msg[len++] = '\n'; msg[len++] = '\n';
        memcpy(msg + len, "  R - Restart\n", 14); len += 14;
        memcpy(msg + len, "  Q - Quit\n", 11); len += 11;
        write(STDOUT_FILENO, msg, len);
    }
    
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
