import sys

def main():
    try:
        for line in sys.stdin:
            print(line, sep=None, end="")
    except:
        exit(1)

if __name__ == "__main__":
    main()