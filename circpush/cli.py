import circpush
import click
# import circpush.server

# print(dir(circpush))
# print("------")
# # print(dir(circpush.server))
# # print("------")
# print(dir(circpush.__package__))
# print(dir(circpush.server))


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
