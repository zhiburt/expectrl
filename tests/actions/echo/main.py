import sys

def main():
    try:
        print(' '.join(sys.argv[1:]))
    except:
        exit(1)

if __name__ == "__main__":
    main()