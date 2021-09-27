#!/usr/bin/env python3

# The file was developed by https://github.com/GaryBoone/

from colors import colorize, Color
import sys
import time
import getpass

NUM_LINES = 5
SAME_LINE_ESC = "\033[F"

def main():
    """Demonstrate several kinds of terminal outputs.

    Examples including ANSI codes, "\r" without "\n", writing to stdin, no-echo
    inputs.
    """

    # Show a color.
    print("status: ", colorize("good", Color.GREEN))

    # Show same-line output via "\r".
    for i in range(NUM_LINES):
        sys.stdout.write(f"[{i+1}/{NUM_LINES}]: file{i}\r")
        time.sleep(1)
    print("\n")

    # Show same-line output via an ANSI code.
    for i in range(NUM_LINES):
        print(f"{SAME_LINE_ESC}[{i+1}/{NUM_LINES}]: file{i}")
        time.sleep(1)

    # Handle prompts which don't repeat input to stdout.
    print("Here is a test password prompt")
    print(colorize("Do not enter a real password", Color.RED))
    getpass.getpass()

    # Handle simple input.
    ans = input("Continue [y/n]:")
    col = Color.GREEN if ans == "y" else Color.RED
    print(f"You said: {colorize(ans, col)}")
    if ans == "n" or ans == "":
        sys.exit(0)

    # Handle long-running process, like starting a server.
    print("[Starting long running process...]")
    print("[Ctrl-C to exit]")
    while True:
        print("status: ", colorize("good", Color.GREEN))
        time.sleep(1)

if __name__ == "__main__":
    main()
