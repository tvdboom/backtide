"""Backtide.

Author: Mavs
Description: Http client for REST API calls.

"""

import asyncio
from typing import Any, Self

from aiohttp import (
    ClientError,
    ClientResponseError,
    ClientSession,
    ClientTimeout,
)


class HttpClient:
    """Async client to call REST API endpoints."""

    MAX_RETRIES = 3
    MAX_TIMEOUT = 3
    SLEEP_TIME = 0.1

    def __init__(self) -> None:
        self._session: ClientSession | None = None

    async def __aenter__(self) -> Self:
        """Async context manager to call REST API endpoints."""
        self._session = ClientSession(headers={"Content-Type": "application/json"})
        return self

    async def __aexit__(self, *_: object):
        """Async context manager to call REST API endpoints."""
        if self._session:
            await self._session.close()
            self._session = None

    async def apiget(self, url: str, params: dict[str, Any] | None = None) -> Any:
        """Call a GET endpoint.

        Calls wait for up to `MAX_TIMEOUT` seconds and are retried `MAX_RETRIES`
        times (with a `SLEEP_TIME` seconds delay).

        Parameters
        ----------
        url : str
            Endpoint to call.

        params : dict[str, Any] | None
            Parameters to pass to the endpoint.

        Returns
        -------
        Any
            JSON decoded response.

        """
        if self._session is None:
            raise RuntimeError("Use the HttpClient as an async context manager.")

        last_exc: Exception | None = None
        for attempt in range(self.MAX_RETRIES):
            try:
                request = self._session.request(
                    method="GET",
                    url=url,
                    params=params,
                    timeout=ClientTimeout(self.MAX_TIMEOUT),
                )

                async with request as response:
                    if response.status >= 500:
                        last_exc = ClientResponseError(
                            response.request_info,
                            response.history,
                            status=response.status,
                        )
                    elif response.status >= 400:
                        response.raise_for_status()
                    else:
                        return await response.json()
            except ClientError as exc:
                last_exc = exc

            if attempt < self.MAX_RETRIES:
                await asyncio.sleep(self.SLEEP_TIME)

        raise last_exc
