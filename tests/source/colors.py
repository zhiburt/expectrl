import enum

class Color(enum.Enum):
    RESET = "\33[0m"
    RED = "\33[31m"
    GREEN = "\33[32m"
    YELLOW = "\33[33m"


def colorize(text: str, color: Color) -> str:
    """Wrap `text` in terminal color directives.

    The return value will show up as the given color when printed in a terminal.
    """
    return f"{color.value}{text}{Color.RESET.value}"
