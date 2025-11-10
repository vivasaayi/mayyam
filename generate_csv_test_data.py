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


import csv
import random
import datetime

RECORDS = 50000
# Field names
field_names = ['id', 'product_name', 'category', 'price', 'last_update_date']

def generate_product_name(i):
    return f"Product_{i}_{random.choice(['Alpha', 'Beta', 'Gamma', 'Delta', 'Epsilon'])}"

def generate_category():
    return random.choice(['Electronics', 'Books', 'Clothing', 'Home Goods', 'Toys'])

def generate_price():
    return round(random.uniform(5.0, 500.0), 2)

def generate_date(base_year=2024, month_range=(1,12), day_range=(1,28)):
    year = base_year + random.randint(0,1) # Allow some variation into the next year for broader date ranges
    month = random.randint(month_range[0], month_range[1])
    day = random.randint(day_range[0], day_range[1])
    # Ensure valid date, e.g. not Feb 30. Simplistic approach here.
    if month == 2 and day > 28:
        day = 28
    elif month in [4,6,9,11] and day > 30:
        day = 30
    return datetime.date(year, month, day).isoformat()

# File 1: Base data
file1_data = []
print(f"Generating {RECORDS} records for file 1...")
for i in range(RECORDS):
    file1_data.append({
        'id': f"key_{i:05}",
        'product_name': generate_product_name(i),
        'category': generate_category(),
        'price': generate_price(),
        'last_update_date': generate_date(2023, (1,12)) # Base year 2023
    })

with open('test_data_file1.csv', 'w', newline='') as csvfile:
    writer = csv.DictWriter(csvfile, fieldnames=field_names)
    writer.writeheader()
    writer.writerows(file1_data)
print(f"Generated test_data_file1.csv with {len(file1_data)} records.")

# File 2: Modified data (some overlap, some changes, some new, some deleted)
file2_data = []
print(f"Generating records for file 2 (based on file 1)...")
ids_file1 = {row['id'] for row in file1_data}
# Keep 80% of original IDs, some of which will be modified
ids_to_keep_modify = random.sample(list(ids_file1), int(RECORDS * 0.8)) 

deleted_count = 0
changed_price_count = 0
changed_category_count = 0

for base_id in ids_to_keep_modify:
    original_row = next(row for row in file1_data if row['id'] == base_id)
    new_row = original_row.copy()
    new_row['last_update_date'] = generate_date(2024, (1,6)) # First half of 2024

    if random.random() < 0.3: # 30% of these change price
        new_row['price'] = round(original_row['price'] * random.uniform(0.8, 1.2), 2)
        if new_row['price'] != original_row['price']: # Ensure actual change
             changed_price_count +=1
    if random.random() < 0.1: # 10% change category
        new_category = generate_category()
        if new_category != original_row['category']:
            new_row['category'] = new_category
            changed_category_count +=1
    file2_data.append(new_row)

# IDs from file1 that are not in ids_to_keep_modify are effectively "deleted" in file2
deleted_count = len(ids_file1) - len(ids_to_keep_modify)

# Add new records (20% of original size)
new_records_count_f2 = 0
for i in range(int(RECORDS * 0.2)):
    new_id_num = RECORDS + i
    file2_data.append({
        'id': f"key_{new_id_num:05}",
        'product_name': generate_product_name(new_id_num),
        'category': generate_category(),
        'price': generate_price(),
        'last_update_date': generate_date(2024, (7,12)) # Second half of 2024
    })
    new_records_count_f2 +=1
random.shuffle(file2_data)

with open('test_data_file2.csv', 'w', newline='') as csvfile:
    writer = csv.DictWriter(csvfile, fieldnames=field_names)
    writer.writeheader()
    writer.writerows(file2_data)
print(f"Generated test_data_file2.csv with {len(file2_data)} records.")
print(f"  File 2 vs File 1: ~{deleted_count} deleted, {changed_price_count} price changed, {changed_category_count} category changed, {new_records_count_f2} added.")

# File 3: Further modified data
file3_data = []
print(f"Generating records for file 3 (based on file 2)...")
ids_file2 = {row['id'] for row in file2_data}
# Keep 90% of file2's IDs, some of which will be modified
ids_to_keep_modify_f3 = random.sample(list(ids_file2), int(len(ids_file2) * 0.90))

deleted_count_f3 = 0
changed_price_count_f3 = 0

for base_id in ids_to_keep_modify_f3:
    original_row = next(row for row in file2_data if row['id'] == base_id)
    new_row = original_row.copy()
    new_row['last_update_date'] = generate_date(2025, (1,3)) # Early 2025

    if random.random() < 0.2: # 20% change price again
        new_price = round(original_row['price'] * random.uniform(0.9, 1.1), 2)
        if new_price != original_row['price']:
            new_row['price'] = new_price
            changed_price_count_f3 +=1
    file3_data.append(new_row)

deleted_count_f3 = len(ids_file2) - len(ids_to_keep_modify_f3)

# Add some completely new records for file3 (10% of original REOCRDS size)
new_records_count_f3 = 0
for i in range(int(RECORDS * 0.1)):
    new_id_num = RECORDS + int(RECORDS * 0.2) + i # Ensure unique IDs from previous batch
    file3_data.append({
        'id': f"key_{new_id_num:05}",
        'product_name': generate_product_name(new_id_num),
        'category': generate_category(),
        'price': generate_price(),
        'last_update_date': generate_date(2025, (2,4))
    })
    new_records_count_f3 +=1
random.shuffle(file3_data)

with open('test_data_file3.csv', 'w', newline='') as csvfile:
    writer = csv.DictWriter(csvfile, fieldnames=field_names)
    writer.writeheader()
    writer.writerows(file3_data)
print(f"Generated test_data_file3.csv with {len(file3_data)} records.")
print(f"  File 3 vs File 2: ~{deleted_count_f3} deleted, {changed_price_count_f3} price changed, {new_records_count_f3} added.")

print("\nScript finished. Test CSV files are in the root directory.")
