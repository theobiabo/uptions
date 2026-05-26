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
  <title>Uptions API Documentation</title>
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
            "description": "Versioned V1 backend endpoints for wallet authentication and Polymarket market discovery."
        },
        "servers": [
            {
                "url": "http://localhost:3000/api/v1",
                "description": "Local development"
            }
        ],
        "paths": {
            "/health": {
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
            "/auth/challenge": {
                "post": {
                    "tags": ["Auth"],
                    "summary": "Create a wallet sign-in challenge",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/CreateChallengeRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Challenge created successfully",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/CreateChallengeResponse"
                                    }
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid wallet address",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
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
                    }
                }
            },
            "/auth/verify": {
                "post": {
                    "tags": ["Auth"],
                    "summary": "Verify a signed wallet challenge",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/VerifyChallengeRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Wallet verified and session issued",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/VerifyChallengeResponse"
                                    }
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid or expired challenge",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
                                    }
                                }
                            }
                        },
                        "401": {
                            "description": "Invalid signature",
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
            "/auth/me": {
                "get": {
                    "tags": ["Auth"],
                    "summary": "Get the current authenticated user",
                    "security": [
                        {
                            "bearerAuth": []
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Current authenticated user",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/AuthUserResponse"
                                    }
                                }
                            }
                        },
                        "401": {
                            "description": "Missing or invalid bearer token",
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
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "Opaque"
                }
            },
            "schemas": {
                "CreateChallengeRequest": {
                    "type": "object",
                    "required": ["wallet_address"],
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "example": "0x1234567890abcdef1234567890abcdef12345678"
                        }
                    }
                },
                "CreateChallengeResponse": {
                    "type": "object",
                    "required": ["wallet_address", "nonce", "message", "expires_at"],
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "example": "0x1234567890abcdef1234567890abcdef12345678"
                        },
                        "nonce": {
                            "type": "string",
                            "example": "550e8400-e29b-41d4-a716-446655440000"
                        },
                        "message": {
                            "type": "string",
                            "example": "Sign in to Uptions\nAddress: 0x1234567890abcdef1234567890abcdef12345678\nNonce: 550e8400-e29b-41d4-a716-446655440000"
                        },
                        "expires_at": {
                            "type": "integer",
                            "format": "uint64",
                            "example": 1760000000
                        }
                    }
                },
                "VerifyChallengeRequest": {
                    "type": "object",
                    "required": ["wallet_address", "signature"],
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "example": "0x1234567890abcdef1234567890abcdef12345678"
                        },
                        "signature": {
                            "type": "string",
                            "example": "0x5f2c9c0d93b1b3fddc55c4f98ccf5281af2c0612fd4f2cfd2c7d4dd4f3838f620dcf54e02db91f7df0ec6ee25b9e6f74fd839cc13a5d08d64f6b3db2de4d6c881b"
                        }
                    }
                },
                "VerifyChallengeResponse": {
                    "type": "object",
                    "required": ["access_token", "token_type", "user"],
                    "properties": {
                        "access_token": {
                            "type": "string",
                            "example": "8c472518-9cfe-4c5b-bb7b-8da1be2aef4d"
                        },
                        "token_type": {
                            "type": "string",
                            "example": "Bearer"
                        },
                        "user": {
                            "$ref": "#/components/schemas/AuthUserResponse"
                        }
                    }
                },
                "AuthUserResponse": {
                    "type": "object",
                    "required": ["wallet_address", "polymarket_linked"],
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "example": "0x1234567890abcdef1234567890abcdef12345678"
                        },
                        "polymarket_linked": {
                            "type": "boolean",
                            "example": false
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
