use std::vec::Vec;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MenuItem {
	pub title: String,
	pub url: String,
	pub target: Option<String>,
	pub children: Option<Vec<MenuItem>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Menu {
	pub id: u16,
	pub name: String,
	pub items: Vec<MenuItem>,
}

impl Menu {
	/// Turns a SQL row into a menu
	pub fn from_sql(mut row: mysql::Row) -> Option<Menu> {
		Some(Menu {
			id: row.take("id")?,
			name: row.take("name")?,
			items: match serde_json::from_str(row.take::<String, _>("items")?.as_str()) {
				Ok(tmp) => { Some(tmp)? }
				_ => { vec![] }
			},
		})
	}
}

/// Load all the menus from the database
pub fn load_menus_from_sql(db: &mysql::Pool) -> Option<Vec<Menu>> {
	let query_result = match db.prep_exec("SELECT id, name, items FROM menus", ()) {
		Ok(tmp) => { tmp }
		_ => { return None; }
	};

	let mut menus = Vec::new();

	for result_row in query_result {
		let row = match result_row {
			Ok(tmp) => { tmp }
			_ => { continue; }
		};

		match Menu::from_sql(row) {
			Some(tmp) => { menus.push(tmp); }
			_ => {}
		}
	}

	Some(menus)
}

/// Create or update a menu in the database
pub fn update_menu_in_sql(db: &mysql::Pool, menu: &Menu) -> u64 {
	let query = r##"
    INSERT INTO menus (id, name, items) VALUES
    (:id, :name, :items)
    ON DUPLICATE KEY UPDATE name=:name, items=:items
    "##;

	let items = match serde_json::to_string(&menu.items) {
		Ok(tmp) => { tmp }
		_ => { String::from("") }
	};

	for mut stmt in db.prepare(query).into_iter() {
		match stmt.execute(params! {"name" => &menu.name, "items" => &items, "id" => menu.id}) {
			Ok(res) => {
				if menu.id > 0 {
					return menu.id as u64;
				}
				return res.last_insert_id();
			}
			_ => {}
		}
	}
	0
}