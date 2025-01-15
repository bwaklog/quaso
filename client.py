#!/Users/grogu/.pyenv/shims/python
import sys
import enum
import socket
from typing import List, Tuple

class Command:
    def __init__(self, key: str) -> None:
        self.key = key

    def get_key(self):
        return self.key

    def builder(self) -> bytes:
        return b""

    def call(self, stream: socket.socket):
        stream.send(self.builder())
        recv = stream.recv(1024).decode()
        print(f"executed {recv.strip()}")
        stream.close()

class GetCommand(Command):
    def __init__(self, key: str) -> None:
        self.key = key
        pass

    def __str__(self) -> str:
        return f"GET | key: {self.key}"

    def builder(self) -> bytes:
        return f"get {self.key}".encode()


class SetCommand(Command):
    def __init__(self, key: str, value: str) -> None:
        self.key = key
        self.value = value

    def __str__(self) -> str:
        return f"SET | key: {self.key}, value: {self.value}"

    def builder(self) -> bytes:
        return f"set {self.key} {self.value}".encode()


class DeleteCommand(Command):
    def __init__(self, key: str) -> None:
        self.key = key
        pass

    def __str__(self) -> str:
        return f"DELETE | key: {self.key}"

    def builder(self) -> bytes:
        return f"delete {self.key}".encode()


class CommandType(enum.Enum):
    SET = 1
    GET = 2
    DELETE = 3
    INVALID = 4

def parse_command_type(cmd: str):
    match cmd.lower():
        case "get":
            return CommandType.GET
        case "set":
            return CommandType.SET
        case "delete":
            return CommandType.DELETE
    return CommandType.INVALID

def validate_arg_length(cmd, args: List[str]) -> Tuple[Command, int]:
    if cmd == CommandType.SET and len(args) != 5:
        print("invalid: SET <KEY> <VALUE> <PORT>")
        exit(1)
    elif cmd in [CommandType.DELETE, CommandType.GET] and len(args) != 4:
        print("invalid: GET/DELETE <KEY> <PORT>")
        exit(1)
    elif cmd == CommandType.INVALID:
        print("invalid: SET/GET/DELETE <KEY> (<VALUE>) <PORT>")
        exit(1)

    match cmd:
        case CommandType.SET:
            return (SetCommand(args[2], args[3]), int(args[4]))
        case CommandType.GET:
            return (GetCommand(args[2]), int(args[3]))
        case CommandType.DELETE:
            return (DeleteCommand(args[2]), int(args[3]))

    return (Command(""), -1)


class CommandExec:
    def __init__(self, command: Command, port: int) -> None:
        self.command = command
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

        self.socket.connect(('localhost', port))
        pass

    def cmd_only(self):
        self.command.call(self.socket)

    def cmd_validate(self):
        self.cmd_only()
        get_cmd = GetCommand(self.command.get_key())

    def close_stream(self):
        self.socket.close()


(command,port) = validate_arg_length(
    parse_command_type(sys.argv[1]), 
    sys.argv
)
cexec = CommandExec(command, port)
cexec.cmd_only()
