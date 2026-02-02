#!/bin/bash
# demo.sh - Demonstration of all utility commands
# Run this script in the target system shell

echo "========================================"
echo "     Utils Command Demonstration"
echo "========================================"
echo ""

# 1. pwd - Print working directory
echo "=== 1. pwd - Print working directory ==="
echo "Command: pwd"
pwd
echo ""

# 2. ls - List directory contents
echo "=== 2. ls - List directory contents ==="
echo "Command: ls"
ls
echo ""
echo "Command: ls -l"
ls -l
echo ""

# 3. echo - Display text
echo "=== 3. echo - Display text ==="
echo "Command: echo \"Hello, World!\""
echo "Hello, World!"
echo ""

# 4. touch - Create empty file
echo "=== 4. touch - Create empty file ==="
echo "Command: touch /tmp/test_file.txt"
touch /tmp/test_file.txt
ls -l /tmp/test_file.txt
echo ""

# 5. cat - Display file contents
echo "=== 5. cat - Display file contents ==="
echo "Hello from cat demo!" > /tmp/test_file.txt
echo "Command: cat /tmp/test_file.txt"
cat /tmp/test_file.txt
echo ""

# 6. cp - Copy files
echo "=== 6. cp - Copy files ==="
echo "Command: cp /tmp/test_file.txt /tmp/test_copy.txt"
cp /tmp/test_file.txt /tmp/test_copy.txt
echo "After copying:"
ls -l /tmp/test_*.txt
echo ""

# 7. mv - Move/rename files
echo "=== 7. mv - Move/rename files ==="
echo "Command: mv /tmp/test_copy.txt /tmp/renamed.txt"
mv /tmp/test_copy.txt /tmp/renamed.txt
ls -l /tmp/*.txt
echo ""

# 8. mkdir - Create directory
echo "=== 8. mkdir - Create directory ==="
echo "Command: mkdir /tmp/demo_dir"
mkdir /tmp/demo_dir
ls -l /tmp/ | grep demo_dir
echo ""

# 9. tree - Display directory tree
echo "=== 9. tree - Display directory tree ==="
touch /tmp/demo_dir/file1.txt
touch /tmp/demo_dir/file2.txt
mkdir /tmp/demo_dir/subdir
touch /tmp/demo_dir/subdir/file3.txt
echo "Command: tree /tmp/demo_dir"
tree /tmp/demo_dir
echo ""

# 10. wc - Count words/lines
echo "=== 10. wc - Count words/lines ==="
echo -e "Line 1\nLine 2\nLine 3" > /tmp/wc_test.txt
echo "Command: wc /tmp/wc_test.txt"
wc /tmp/wc_test.txt
echo ""

# 11. hexdump - Hex view
echo "=== 11. hexdump - Hex view of file ==="
echo "Command: hexdump /tmp/test_file.txt"
hexdump /tmp/test_file.txt
echo ""

# 12. cal - Display calendar
echo "=== 12. cal - Display calendar ==="
echo "Command: cal"
cal
echo ""

# 13. uptime - Display system uptime
echo "=== 13. uptime - Display system uptime ==="
echo "Command: uptime"
uptime
echo ""

# 14. top - System monitor
echo "=== 14. top - System resource monitor ==="
echo "Command: top -n 1 (run once then exit)"
echo "Press 'q' to quit top if needed"
top -n 1
echo ""

# 15. rm - Remove files
echo "=== 15. rm - Remove files ==="
echo "Command: rm /tmp/test_file.txt"
rm /tmp/test_file.txt
rm /tmp/renamed.txt
rm /tmp/wc_test.txt
rm -r /tmp/demo_dir
echo "Cleanup completed"
echo ""

# Summary
echo "========================================"
echo "     Demo Complete!"
echo "========================================"
echo ""
echo "Available tools:"
echo "  pwd      - Print working directory"
echo "  ls       - List directory contents"
echo "  echo     - Display text"
echo "  touch    - Create empty file"
echo "  cat      - Display file contents"
echo "  cp       - Copy files"
echo "  mv       - Move/rename files"
echo "  mkdir    - Create directory"
echo "  rm       - Remove files"
echo "  tree     - Display directory tree"
echo "  wc       - Count words/lines"
echo "  hexdump  - Hex view of file"
echo "  cal      - Display calendar"
echo "  uptime   - Display system uptime"
echo "  top      - System monitor (press q to quit)"
echo ""
