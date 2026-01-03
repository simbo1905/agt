#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
DECK_DIR="$PROJECT_ROOT/deck"

PSQL="/opt/homebrew/opt/postgresql@14/bin/psql"
CREATEDB="/opt/homebrew/opt/postgresql@14/bin/createdb"
OPENRESTY="/opt/homebrew/opt/openresty/bin/openresty"
OPM="/opt/homebrew/opt/openresty/bin/opm"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

check_postgres() {
    log_info "Checking PostgreSQL..."
    if ! brew services list | grep -q "postgresql@14.*started"; then
        log_warn "PostgreSQL not running. Starting..."
        brew services start postgresql@14
        sleep 2
    fi
    log_info "PostgreSQL is running"
}

check_openresty() {
    log_info "Checking OpenResty..."
    if [ ! -x "$OPENRESTY" ]; then
        log_error "OpenResty not found at $OPENRESTY"
        log_info "Install with: brew install openresty/brew/openresty"
        exit 1
    fi
    log_info "OpenResty found"
}

install_pgmoon() {
    log_info "Checking pgmoon..."
    if [ ! -d "/opt/homebrew/opt/openresty/site/lualib/pgmoon" ]; then
        log_info "Installing pgmoon..."
        "$OPM" get leafo/pgmoon
    fi
    log_info "pgmoon installed"
}

setup_database() {
    log_info "Setting up database..."
    "$CREATEDB" agt_deck 2>/dev/null || log_warn "Database already exists"
    "$PSQL" agt_deck < "$DECK_DIR/sql/schema.sql"
    log_info "Database schema applied"
}

start_openresty() {
    log_info "Starting OpenResty..."
    cd "$PROJECT_ROOT"
    
    if pgrep -f "nginx.*deck/conf/nginx.conf" > /dev/null; then
        log_warn "OpenResty already running, reloading..."
        "$OPENRESTY" -p "$DECK_DIR" -c conf/nginx.conf -s reload
    else
        "$OPENRESTY" -p "$DECK_DIR" -c conf/nginx.conf
    fi
    
    log_info "OpenResty started on http://localhost:8080"
}

stop_openresty() {
    log_info "Stopping OpenResty..."
    cd "$PROJECT_ROOT"
    "$OPENRESTY" -p "$DECK_DIR" -c conf/nginx.conf -s stop 2>/dev/null || true
    log_info "OpenResty stopped"
}

test_api() {
    log_info "Testing API endpoints..."
    
    sleep 1
    
    if curl -s http://localhost:8080/api/projects | grep -q '\['; then
        log_info "GET /api/projects - OK"
    else
        log_error "GET /api/projects - FAILED"
        return 1
    fi
    
    if curl -s http://localhost:8080/api/sessions | grep -q '\['; then
        log_info "GET /api/sessions - OK"
    else
        log_error "GET /api/sessions - FAILED"
        return 1
    fi
    
    if curl -s http://localhost:8080/ | grep -q 'OpenCode Deck'; then
        log_info "GET / (SPA) - OK"
    else
        log_error "GET / (SPA) - FAILED"
        return 1
    fi
    
    log_info "All API tests passed"
}

show_logs() {
    log_info "Tailing logs..."
    tail -f "$DECK_DIR/logs/error.log" "$DECK_DIR/logs/access.log"
}

case "${1:-setup}" in
    setup)
        check_postgres
        check_openresty
        install_pgmoon
        setup_database
        start_openresty
        test_api
        log_info "Setup complete! Open http://localhost:8080"
        ;;
    start)
        start_openresty
        ;;
    stop)
        stop_openresty
        ;;
    restart)
        stop_openresty
        sleep 1
        start_openresty
        ;;
    test)
        test_api
        ;;
    logs)
        show_logs
        ;;
    status)
        if pgrep -f "nginx.*deck/conf/nginx.conf" > /dev/null; then
            log_info "OpenResty is running"
            pgrep -f "nginx.*deck/conf/nginx.conf"
        else
            log_warn "OpenResty is not running"
        fi
        ;;
    *)
        echo "Usage: $0 {setup|start|stop|restart|test|logs|status}"
        exit 1
        ;;
esac
