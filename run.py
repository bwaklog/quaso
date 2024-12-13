#!/Library/Frameworks/Python.framework/Versions/3.12/bin/python3
import sys
import subprocess
import signal
import inspect
import asyncio


NODE_NAME = sys.argv[1:]
command = [
    "cargo",
    "run",
    "--release",
    "-q",
    "-p",
    "kv",
    "--",
    "-c",
    f"./kv/tmp/conf_{NODE_NAME[0]}.yml",
]


async def start_node(command):
    subprocess.run(command)


asyncio.run(start_node(command))
