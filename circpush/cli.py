import circpush
import click
import os
import json
import tabulate
import os.path
import sys
from datetime import datetime


@click.group()
def cli() -> None:
    """Main CLI entry."""

@cli.group()
def server() -> None:
    """Server subcommand group."""

@server.command(name="start")
def server_start() -> None:
    """Server start command."""
    circpush.server.start()

@server.command(name="run")
def server_run() -> None:
    """Server run command."""
    circpush.server.run()

@server.command(name="stop")
def server_stop() -> None:
    """Server stop command."""
    circpush.client.stop_server()

@cli.command()
def ping() -> None:
    """Ping server command."""
    response = circpush.client.ping()
    click.echo(response)

@cli.command()
@click.argument("message")
def echo(message: str) -> None:
    """Ping server command."""
    response = circpush.client.echo(message)
    click.echo(response)

@cli.command(name="start")
@click.argument("read_pattern")
@click.argument("write_directory")
def link_start(read_pattern: str, write_directory: str) -> None:
    """Start link command."""
    base_directory = os.getcwd()
    response = circpush.client.start_link(
        read_pattern,
        write_directory,
        base_directory,
    )
    click.echo(response)

@cli.command(name="stop")
@click.argument("number", type=int, default=0)
def link_stop(number: int) -> None:
    """Stop link command."""
    try:
        response = circpush.client.stop_link(number)
        click.echo(response)
    except Exception as err:
        click.echo(f"{err} {number}")

@cli.command(name="view")
@click.argument("number", type=int, default=0)
@click.option("--absolute", is_flag=True, default=False)
def link_view(number: int, absolute: bool) -> None:
    """View link command."""
    if number < 0:
        click.echo("Link number must be between 1 and the total number of links, or 0/blank for all links")
        sys.exit(1)

    try:
        response = circpush.client.view_link(number)
        links = json.loads(response)
        if number != 0:
            links = [links]

        headers = ("#", "Read Pattern", "Base Directory", "Write Directory")
        table = []
        for index, link in enumerate(links):
            base_directory = link["base_directory"]
            base_directory = base_directory if absolute else os.path.relpath(base_directory)
            write_directory = link["write_directory"]
            write_directory = write_directory if absolute else os.path.relpath(write_directory)
            table.append((
                index + 1,
                link["read_pattern"],
                base_directory,
                write_directory,
            ))
        table_text = tabulate.tabulate(table, headers=headers)
        click.echo(table_text)
    except Exception as err:
        click.echo(f"{err} {number}")
        sys.exit(1)

@cli.command(name="ledger")
@click.option("--absolute", is_flag=True, default=False)
def link_ledger(absolute: bool) -> None:
    """View file ledger command."""
    response = circpush.client.view_link(0)
    links = json.loads(response)

    headers = ("Source", "Destination")
    fileset = set()
    for link in links:
        for filelink in link["links"]:
            source = filelink["source"]
            source = source if absolute else os.path.relpath(source)
            destination = filelink["destination"]
            destination = destination if absolute else os.path.relpath(destination)
            fileset.add((source, destination))
    table_text = tabulate.tabulate(sorted(fileset), headers=headers)
    click.echo(table_text)
