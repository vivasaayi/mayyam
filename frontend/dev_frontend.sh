# Copyright (c) 2025 Rajan Panneer Selvam
#
# Licensed under the Business Source License 1.1 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     https://www.mariadb.com/bsl11
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.


#!/bin/bash

# Frontend development runner
# Usage: ./dev_frontend.sh [backend_port] [frontend_port]

source ../.env

echo "Starting frontend on port $FRONTEND_PORT..."
echo "Connecting to backend on port $BACKEND_PORT..."


REACT_APP_BACKEND_PORT=$BACKEND_PORT REACT_APP_API_URL=http://localhost:$BACKEND_PORT PORT=$FRONTEND_PORT npm start
