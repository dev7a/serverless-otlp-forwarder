"""Tests for event extractors using SAM-generated fixtures."""

import json
from pathlib import Path
from typing import Any

import pytest
from opentelemetry.trace import SpanKind

from lambda_otel_lite.extractors import (
    TriggerType,
    alb_extractor,
    api_gateway_v1_extractor,
    api_gateway_v2_extractor,
    default_extractor,
)

# Load fixtures
FIXTURES_DIR = Path(__file__).parent / "fixtures"


def load_fixture(name: str) -> dict[str, Any]:
    """Load a test fixture from the fixtures directory."""
    with open(FIXTURES_DIR / name) as f:
        return json.load(f)


# Load all fixtures once
FIXTURES = {
    "apigw_v1": load_fixture("apigw_v1_proxy.json"),
    "apigw_v2": load_fixture("apigw_v2_proxy.json"),
    "alb": load_fixture("alb.json"),
}


@pytest.fixture
def lambda_context():
    """Create a mock Lambda context with all required attributes."""

    class MockContext:
        aws_request_id = "test-request-id"
        invoked_function_arn = (
            "arn:aws:lambda:us-west-2:123456789012:function:test-function"
        )
        function_name = "test-function"
        function_version = "$LATEST"
        memory_limit_in_mb = 128
        log_stream_name = "2024/02/16/[$LATEST]1234567890"

    return MockContext()


class TestExtractors:
    """Test event extractors with SAM-generated fixtures."""

    def test_api_gateway_v1(self) -> None:
        """Test API Gateway v1 extractor with SAM fixture."""
        event = FIXTURES["apigw_v1"]
        result = api_gateway_v1_extractor(event, None)

        assert result.trigger == TriggerType.HTTP
        assert result.kind == SpanKind.SERVER
        assert result.carrier == event.get("headers")

        # Check extracted attributes
        attrs = result.attributes
        assert attrs["http.request.method"] == event["httpMethod"]
        assert attrs["url.path"] == event["path"]
        assert attrs["url.scheme"] == "https"
        assert attrs["http.route"] == event["resource"]
        assert result.span_name == f"{event['httpMethod']} {event['resource']}"

        # Check headers are normalized
        if headers := event.get("headers"):
            if "User-Agent" in headers:
                assert attrs["user_agent.original"] == headers["User-Agent"]
            elif "user-agent" in headers:
                assert attrs["user_agent.original"] == headers["user-agent"]

    def test_api_gateway_v2(self) -> None:
        """Test API Gateway v2 extractor with SAM fixture."""
        event = FIXTURES["apigw_v2"]
        result = api_gateway_v2_extractor(event, None)

        route = event["routeKey"]
        if route == "$default":
            route = event.get("rawPath", "/")

        assert result.trigger == TriggerType.HTTP
        assert result.kind == SpanKind.SERVER
        assert result.carrier == event.get("headers")

        # Check extracted attributes
        attrs = result.attributes
        assert attrs["http.request.method"] == event["requestContext"]["http"]["method"]
        assert attrs["url.path"] == event["rawPath"]
        assert attrs["url.scheme"] == "https"
        assert attrs["http.route"] == route
        assert (
            result.span_name == f"{event['requestContext']['http']['method']} {route}"
        )

        # Check headers are normalized
        if headers := event.get("headers"):
            if "User-Agent" in headers:
                assert attrs["user_agent.original"] == headers["User-Agent"]
            elif "user-agent" in headers:
                assert attrs["user_agent.original"] == headers["user-agent"]

    def test_alb(self) -> None:
        """Test ALB extractor with SAM fixture."""
        event = FIXTURES["alb"]
        result = alb_extractor(event, None)

        assert result.trigger == TriggerType.HTTP
        assert result.kind == SpanKind.SERVER
        assert result.carrier == event.get("headers")

        # Check extracted attributes
        attrs = result.attributes
        assert attrs["http.request.method"] == event["httpMethod"]
        assert attrs["url.path"] == event["path"]
        assert attrs["url.scheme"] == "http"
        assert attrs["http.route"] == event["path"]
        assert attrs["network.protocol.version"] == "1.1"
        assert result.span_name == f"{event['httpMethod']} {event['path']}"
        # Check headers are normalized
        if headers := event.get("headers"):
            if "User-Agent" in headers:
                assert attrs["user_agent.original"] == headers["User-Agent"]
            elif "user-agent" in headers:
                assert attrs["user_agent.original"] == headers["user-agent"]

            # Check X-Forwarded-For handling
            if "X-Forwarded-For" in headers:
                client_ip = headers["X-Forwarded-For"].split(",")[0].strip()
                assert attrs["client.address"] == client_ip
            elif "x-forwarded-for" in headers:
                client_ip = headers["x-forwarded-for"].split(",")[0].strip()
                assert attrs["client.address"] == client_ip

    def test_default_extractor(self, lambda_context) -> None:
        """Test default extractor with Lambda context."""
        result = default_extractor({}, lambda_context)

        assert result.trigger == TriggerType.OTHER
        assert result.kind == SpanKind.SERVER
        assert result.attributes == {
            "faas.invocation_id": lambda_context.aws_request_id,
            "cloud.resource_id": lambda_context.invoked_function_arn,
            "cloud.account.id": "123456789012",
        }

    def test_missing_data(self) -> None:
        """Test extractors handle missing data gracefully."""
        empty_event = {}

        # API Gateway v1
        v1_result = api_gateway_v1_extractor(empty_event, None)
        assert v1_result.trigger == TriggerType.HTTP
        assert v1_result.kind == SpanKind.SERVER
        assert "url.scheme" in v1_result.attributes
        assert v1_result.attributes["url.scheme"] == "https"

        # API Gateway v2
        v2_result = api_gateway_v2_extractor(empty_event, None)
        assert v2_result.trigger == TriggerType.HTTP
        assert v2_result.kind == SpanKind.SERVER
        assert "url.scheme" in v2_result.attributes
        assert v2_result.attributes["url.scheme"] == "https"

        # ALB
        alb_result = alb_extractor(empty_event, None)
        assert alb_result.trigger == TriggerType.HTTP
        assert alb_result.kind == SpanKind.SERVER
        assert "url.scheme" in alb_result.attributes
        assert alb_result.attributes["url.scheme"] == "http"

    def test_mixed_case_headers(self) -> None:
        """Test that headers are handled case-insensitively."""
        # Test API Gateway v1 with fixture
        v1_event = FIXTURES["apigw_v1"]
        result = api_gateway_v1_extractor(v1_event, None)
        attrs = result.attributes
        assert "user_agent.original" in attrs
        assert (
            attrs["user_agent.original"]
            == v1_event["requestContext"]["identity"]["userAgent"]
        )

        # Test API Gateway v2 with fixture
        v2_event = FIXTURES["apigw_v2"]
        result = api_gateway_v2_extractor(v2_event, None)
        attrs = result.attributes
        assert "user_agent.original" in attrs
        assert (
            attrs["user_agent.original"]
            == v2_event["requestContext"]["http"]["userAgent"]
        )

        # Test ALB with fixture
        alb_event = FIXTURES["alb"]
        result = alb_extractor(alb_event, None)
        attrs = result.attributes

        # ALB gets user agent from headers
        assert "user_agent.original" in attrs
        assert attrs["user_agent.original"] == alb_event["headers"]["user-agent"]

        # ALB processes these header values
        assert "client.address" in attrs
        assert (
            attrs["client.address"]
            == alb_event["headers"]["x-forwarded-for"].split(",")[0].strip()
        )
        assert "server.address" in attrs
        assert attrs["server.address"] == alb_event["headers"]["host"]
        assert attrs["url.scheme"] == alb_event["headers"]["x-forwarded-proto"]
