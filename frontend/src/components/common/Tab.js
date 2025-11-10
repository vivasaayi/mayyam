// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


import React from 'react';

const Tab = ({ label, isActive, onClick }) => (
    <button
        style={{
            padding: '10px 15px', // Adjusted padding for a common use case
            marginRight: '5px',
            border: '1px solid #ccc',
            borderBottom: isActive ? 'none' : '1px solid #ccc',
            backgroundColor: isActive ? 'white' : '#f0f0f0',
            cursor: 'pointer'
        }}
        onClick={onClick}
    >
        {label}
    </button>
);

export default Tab;
