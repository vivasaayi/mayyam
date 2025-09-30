#!/bin/bash

# Mayyam Docker Compose Runner
# Simplifies running different deployment modes

set -e

# Source environment variables from .env file
source .env

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Function to show usage
show_usage() {
    echo "Mayyam Docker Compose Runner"
    echo ""
    echo "Usage: $0 [MODE] [COMMAND]"
    echo ""
    echo "MODES:"
    echo "  dev     Development mode with hot reloading"
    echo "  uat     UAT mode for testing"
    echo "  prod    Production mode (single container)"
    echo ""
    echo "COMMANDS:"
    echo "  up      Start services"
    echo "  down    Stop services"
    echo "  build   Build services"
    echo "  logs    Show logs"
    echo "  test    Run integration tests"
    echo "  shell   Open shell in backend container"
    echo ""
    echo "EXAMPLES:"
    echo "  $0 dev up        # Start development environment"
    echo "  $0 uat test      # Run tests in UAT mode"
    echo "  $0 prod up       # Start production environment"
    echo "  $0 dev logs      # Show dev logs"
    echo "  $0 down          # Stop all services"
}

# Function to check if docker and docker-compose are available
check_dependencies() {
    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed or not in PATH"
        exit 1
    fi

    if ! command -v docker-compose &> /dev/null; then
        print_error "Docker Compose is not installed or not in PATH"
        exit 1
    fi
}

# Function to run docker-compose commands
run_compose() {
    local mode=$1
    local command=$2
    local extra_args=${3:-""}

    case $mode in
        dev)
            COMPOSE_CMD="docker-compose --profile dev"
            ;;
        uat)
            COMPOSE_CMD="docker-compose --profile uat"
            ;;
        prod)
            COMPOSE_CMD="docker-compose -f docker-compose.prod.yml"
            ;;
        *)
            print_error "Invalid mode: $mode"
            show_usage
            exit 1
            ;;
    esac

    case $command in
        up)
            print_info "Starting $mode environment..."
            $COMPOSE_CMD up -d --build $extra_args
            print_success "$mode environment started!"
            show_access_info $mode
            ;;
        down)
            print_info "Stopping $mode environment..."
            $COMPOSE_CMD down $extra_args
            print_success "$mode environment stopped!"
            ;;
        build)
            print_info "Building $mode environment..."
            $COMPOSE_CMD build $extra_args
            print_success "$mode environment built!"
            ;;
        logs)
            print_info "Showing logs for $mode environment..."
            $COMPOSE_CMD logs -f $extra_args
            ;;
        test)
            if [ "$mode" = "prod" ]; then
                print_error "Testing is not available in production mode"
                exit 1
            fi
            print_info "Running integration tests in $mode mode..."
            $COMPOSE_CMD up --build integration-tests
            ;;
        shell)
            if [ "$mode" = "prod" ]; then
                print_error "Shell access is not available in production mode"
                exit 1
            fi
            print_info "Opening shell in backend container..."
            service_name="backend-${mode}"
            $COMPOSE_CMD exec $service_name /bin/bash
            ;;
        *)
            print_error "Invalid command: $command"
            show_usage
            exit 1
            ;;
    esac
}

# Function to show access information
show_access_info() {
    local mode=$1

    echo ""
    print_success "Services are starting up..."
    echo ""

    if [ "$mode" = "prod" ]; then
        echo "🌐 Application: http://localhost"
        echo "🏥 Health Check: Built-in monitoring"
    else
        echo "🌐 Frontend:     http://localhost:${FRONTEND_PORT:-3000}"
        echo "🔌 Backend API:  http://localhost:${BACKEND_PORT:-8080}"
        echo "🗄️  PHPMyAdmin:   http://localhost:${PHPMYADMIN_PORT:-8081}"
        echo "📨 Kafka:        localhost:${KAFKA_PORT:-9092}"
        echo "🐘 PostgreSQL:   localhost:${POSTGRES_PORT:-5432}"
        echo "🦭 MySQL:        localhost:${MYSQL_PORT:-3306}"
    fi

    echo ""
    print_info "Use '$0 $mode logs' to see startup logs"
}

# Main script logic
main() {
    check_dependencies

    if [ $# -eq 0 ]; then
        show_usage
        exit 0
    fi

    local mode=""
    local command=""

    # Parse arguments
    case $1 in
        dev|uat|prod)
            mode=$1
            shift
            ;;
        down)
            # Special case: down works for all modes
            command=$1
            shift
            ;;
        *)
            print_error "Invalid mode: $1"
            show_usage
            exit 1
            ;;
    esac

    if [ -z "$command" ]; then
        if [ $# -eq 0 ]; then
            command="up"
        else
            command=$1
            shift
        fi
    fi

    # Handle the down command specially
    if [ "$command" = "down" ]; then
        if [ -n "$mode" ]; then
            run_compose $mode $command "$@"
        else
            # Stop all profiles
            print_info "Stopping all environments..."
            docker-compose --profile dev down "$@"
            docker-compose --profile uat down "$@"
            docker-compose -f docker-compose.prod.yml down "$@"
            print_success "All environments stopped!"
        fi
        exit 0
    fi

    if [ -z "$mode" ]; then
        print_error "Mode is required for this command"
        show_usage
        exit 1
    fi

    run_compose $mode $command "$@"
}

# Run main function with all arguments
main "$@"
