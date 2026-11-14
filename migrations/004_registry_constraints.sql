CREATE UNIQUE INDEX projects_name_unique_idx ON projects (LOWER(name));
CREATE UNIQUE INDEX environments_project_name_unique_idx ON environments (project_id, LOWER(name));
CREATE UNIQUE INDEX scenarios_project_name_version_unique_idx ON scenarios (project_id, LOWER(name), version);
