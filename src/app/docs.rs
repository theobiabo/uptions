use axum::{
    Json,
    response::{Html, IntoResponse},
};
use serde_json::{Value, json};

pub async fn openapi_json() -> impl IntoResponse {
    Json(build_openapi_document())
}

pub async fn swagger_ui() -> impl IntoResponse {
    Html(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Uptions API Docs</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.ui = SwaggerUIBundle({
      url: "/docs/openapi.json",
      dom_id: "#swagger-ui"
    });
  </script>
</body>
</html>"##,
    )
}

fn build_openapi_document() -> Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Uptions Backend API",
            "version": "1.0.0",
            "description": "Backend endpoints for Polymarket authentication and market discovery."
        },
        "servers": [
            {
                "url": "http://localhost:3000",
                "description": "Local development"
            }
        ],
        "paths": {
            "/": {
                "get": {
                    "tags": ["Health"],
                    "summary": "Health check",
                    "responses": {
                        "200": {
                            "description": "Application is healthy",
                            "content": {
                                "text/plain": {
                                    "schema": {
                                        "type": "string",
                                        "example": "Uptions endpoint is running"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/polymarket/auth": {
                "post": {
                    "tags": ["Polymarket"],
                    "summary": "Create or derive Polymarket API credentials",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/PolymarketAuthRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Polymarket credentials created or derived successfully",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/PolymarketAuthResponse"
                                    }
                                }
                            }
                        },
                        "500": {
                            "description": "Server or configuration failure",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
                                    }
                                }
                            }
                        },
                        "502": {
                            "description": "Upstream Polymarket error",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/polymarket/markets": {
                "get": {
                    "tags": ["Polymarket"],
                    "summary": "Fetch Polymarket markets",
                    "parameters": [
                        {
                            "name": "limit",
                            "in": "query",
                            "schema": { "type": "integer", "format": "uint32", "minimum": 1 },
                            "required": false
                        },
                        {
                            "name": "offset",
                            "in": "query",
                            "schema": { "type": "integer", "format": "uint32", "minimum": 0 },
                            "required": false
                        },
                        {
                            "name": "active",
                            "in": "query",
                            "schema": { "type": "boolean" },
                            "required": false
                        },
                        {
                            "name": "closed",
                            "in": "query",
                            "schema": { "type": "boolean" },
                            "required": false
                        },
                        {
                            "name": "archived",
                            "in": "query",
                            "schema": { "type": "boolean" },
                            "required": false
                        },
                        {
                            "name": "slug",
                            "in": "query",
                            "schema": { "type": "string" },
                            "required": false
                        },
                        {
                            "name": "id",
                            "in": "query",
                            "schema": { "type": "string" },
                            "required": false
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Raw Polymarket markets payload",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/MarketsResponse"
                                    }
                                }
                            }
                        },
                        "502": {
                            "description": "Upstream Polymarket error",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "PolymarketAuthRequest": {
                    "type": "object",
                    "properties": {
                        "nonce": {
                            "type": "integer",
                            "format": "uint32",
                            "nullable": true,
                            "example": 0
                        }
                    }
                },
                "PolymarketAuthResponse": {
                    "type": "object",
                    "required": ["address", "apiKey", "secret", "passphrase"],
                    "properties": {
                        "address": {
                            "type": "string",
                            "example": "0x1234567890abcdef1234567890abcdef12345678"
                        },
                        "apiKey": {
                            "type": "string",
                            "example": "550e8400-e29b-41d4-a716-446655440000"
                        },
                        "secret": {
                            "type": "string",
                            "example": "base64EncodedSecretString"
                        },
                        "passphrase": {
                            "type": "string",
                            "example": "randomPassphraseString"
                        }
                    }
                },
                "ErrorResponse": {
                    "type": "object",
                    "required": ["success", "message"],
                    "properties": {
                        "success": {
                            "type": "boolean",
                            "example": false
                        },
                        "message": {
                            "type": "string",
                            "example": "External API error: invalid request"
                        }
                    }
                },
                "MarketsResponse": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": true
                    },
                    "example": [
                        {
                            "id": "12345",
                            "question": "Will BTC be above $100k by year end?",
                            "slug": "btc-above-100k-by-year-end",
                            "active": true,
                            "closed": false
                        }
                    ]
                }
            }
        }
    })
}
