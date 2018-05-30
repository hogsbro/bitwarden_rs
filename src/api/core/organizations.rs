#![allow(unused_imports)]

use rocket_contrib::{Json, Value};

use db::DbConn;
use db::models::*;

use api::{PasswordData, JsonResult, EmptyResult, NumberOrString};
use auth::{Headers, AdminHeaders, OwnerHeaders};


#[derive(Deserialize)]
#[allow(non_snake_case)]
struct OrgData {
    billingEmail: String,
    collectionName: String,
    key: String,
    name: String,
    #[serde(rename = "planType")]
    _planType: String, // Ignored, always use the same plan
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct OrganizationUpdateData {
    billingEmail: String,
    name: String,
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct NewCollectionData {
    name: String,
}

#[post("/organizations", data = "<data>")]
fn create_organization(headers: Headers, data: Json<OrgData>, conn: DbConn) -> JsonResult {
    let data: OrgData = data.into_inner();

    let mut org = Organization::new(data.name, data.billingEmail);
    let mut user_org = UserOrganization::new(
        headers.user.uuid.clone(), org.uuid.clone());
    let mut collection = Collection::new(
        org.uuid.clone(), data.collectionName);

    user_org.key = data.key;
    user_org.access_all = true;
    user_org.type_ = UserOrgType::Owner as i32;
    user_org.status = UserOrgStatus::Confirmed as i32;

    org.save(&conn);
    user_org.save(&conn);
    collection.save(&conn);

    Ok(Json(org.to_json()))
}

#[post("/organizations/<org_id>/delete", data = "<data>")]
fn delete_organization(org_id: String, data: Json<PasswordData>, headers: OwnerHeaders, conn: DbConn) -> EmptyResult {
    let data: PasswordData = data.into_inner();
    let password_hash = data.masterPasswordHash;

    if !headers.user.check_valid_password(&password_hash) {
        err!("Invalid password")
    }

    match Organization::find_by_uuid(&org_id, &conn) {
        None => err!("Organization not found"),
        Some(org) => match org.delete(&conn) {
            Ok(()) => Ok(()),
            Err(_) => err!("Failed deleting the organization")
        }
    }
}

#[get("/organizations/<org_id>")]
fn get_organization(org_id: String, _headers: OwnerHeaders, conn: DbConn) -> JsonResult {
    match Organization::find_by_uuid(&org_id, &conn) {
        Some(organization) => Ok(Json(organization.to_json())),
        None => err!("Can't find organization details")
    }
}

#[post("/organizations/<org_id>", data = "<data>")]
fn post_organization(org_id: String, _headers: OwnerHeaders, data: Json<OrganizationUpdateData>, conn: DbConn) -> JsonResult {
    let data: OrganizationUpdateData = data.into_inner();

    let mut org = match Organization::find_by_uuid(&org_id, &conn) {
        Some(organization) => organization,
        None => err!("Can't find organization details")
    };

    org.name = data.name;
    org.billing_email = data.billingEmail;
    org.save(&conn);

    Ok(Json(org.to_json()))
}

// GET /api/collections?writeOnly=false
#[get("/collections")]
fn get_user_collections(headers: Headers, conn: DbConn) -> JsonResult {

    Ok(Json(json!({
        "Data":
            Collection::find_by_user_uuid(&headers.user.uuid, &conn)
            .iter()
            .map(|collection| {
                collection.to_json()
            }).collect::<Value>(),
        "Object": "list"
    })))
}

#[get("/organizations/<org_id>/collections")]
fn get_org_collections(org_id: String, _headers: AdminHeaders, conn: DbConn) -> JsonResult {
    Ok(Json(json!({
        "Data":
            Collection::find_by_organization(&org_id, &conn)
            .iter()
            .map(|collection| {
                collection.to_json()
            }).collect::<Value>(),
        "Object": "list"
    })))
}

#[post("/organizations/<org_id>/collections", data = "<data>")]
fn post_organization_collections(org_id: String, _headers: AdminHeaders, data: Json<NewCollectionData>, conn: DbConn) -> JsonResult {
    let data: NewCollectionData = data.into_inner();

    let org = match Organization::find_by_uuid(&org_id, &conn) {
        Some(organization) => organization,
        None => err!("Can't find organization details")
    };

    let mut collection = Collection::new(org.uuid.clone(), data.name);

    collection.save(&conn);

    Ok(Json(collection.to_json()))
}

#[post("/organizations/<org_id>/collections/<col_id>", data = "<data>")]
fn post_organization_collection_update(org_id: String, col_id: String, _headers: AdminHeaders, data: Json<NewCollectionData>, conn: DbConn) -> JsonResult {
    let data: NewCollectionData = data.into_inner();

    let org = match Organization::find_by_uuid(&org_id, &conn) {
        Some(organization) => organization,
        None => err!("Can't find organization details")
    };

    let mut collection = match Collection::find_by_uuid(&col_id, &conn) {
        Some(collection) => collection,
        None => err!("Collection not found")
    };

    if collection.org_uuid != org.uuid {
        err!("Collection is not owned by organization");
    }

    collection.name = data.name.clone();
    collection.save(&conn);

    Ok(Json(collection.to_json()))
}

#[post("/organizations/<org_id>/collections/<col_id>/delete-user/<org_user_id>")]
fn post_organization_collection_delete_user(org_id: String, col_id: String, org_user_id: String, _headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let collection = match Collection::find_by_uuid(&col_id, &conn) {
        None => err!("Collection not found"),
        Some(collection) => if collection.org_uuid == org_id {
            collection
        } else {
            err!("Collection and Organization id do not match")
        }
    };

    match UserOrganization::find_by_uuid(&org_user_id, &conn) {
        None => err!("User not found in organization"),
        Some(user_org) => {
            match CollectionUser::find_by_collection_and_user(&collection.uuid, &user_org.user_uuid, &conn) {
                None => err!("User not assigned to collection"),
                Some(col_user) => {
                    match col_user.delete(&conn) {
                        Ok(()) => Ok(()),
                        Err(_) => err!("Failed removing user from collection")
                    }
                }
            }
        }
    }
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct DeleteCollectionData {
    id: String,
    orgId: String,
}

#[post("/organizations/<org_id>/collections/<col_id>/delete", data = "<data>")]
fn post_organization_collection_delete(org_id: String, col_id: String, _headers: AdminHeaders, data: Json<DeleteCollectionData>, conn: DbConn) -> EmptyResult {
    let _data: DeleteCollectionData = data.into_inner();

    match Collection::find_by_uuid(&col_id, &conn) {
        None => err!("Collection not found"),
        Some(collection) => if collection.org_uuid == org_id {
            match collection.delete(&conn) {
                Ok(()) => Ok(()),
                Err(_) => err!("Failed deleting collection")
            }
        } else {
            err!("Collection and Organization id do not match")
        }
    }
}

#[get("/organizations/<org_id>/collections/<coll_id>/details")]
fn get_org_collection_detail(org_id: String, coll_id: String, headers: AdminHeaders, conn: DbConn) -> JsonResult {
    match Collection::find_by_uuid_and_user(&coll_id, &headers.user.uuid, &conn) {
        None => err!("Collection not found"),
        Some(collection) => {
            if collection.org_uuid != org_id {
                err!("Collection is not owned by organization")
            }

            Ok(Json(collection.to_json()))
        }
    }
}

#[get("/organizations/<org_id>/collections/<coll_id>/users")]
fn get_collection_users(org_id: String, coll_id: String, _headers: AdminHeaders, conn: DbConn) -> JsonResult {
    // Get org and collection, check that collection is from org
    let collection = match Collection::find_by_uuid_and_org(&coll_id, &org_id, &conn) {
        None => err!("Collection not found in Organization"),
        Some(collection) => collection
    };

    // Get the users from collection
    let user_list: Vec<Value> = CollectionUser::find_by_collection(&collection.uuid, &conn)
    .iter().map(|col_user|  {
        UserOrganization::find_by_user_and_org(&col_user.user_uuid, &org_id, &conn)
        .unwrap()
        .to_json_collection_user_details(&col_user.read_only, &conn)
    }).collect();

    Ok(Json(json!({
        "Data": user_list,
        "Object": "list"
    })))
}

#[derive(FromForm)]
#[allow(non_snake_case)]
struct OrgIdData {
    organizationId: String
}

#[get("/ciphers/organization-details?<data>")]
fn get_org_details(data: OrgIdData, headers: Headers, conn: DbConn) -> JsonResult {
    let ciphers = Cipher::find_by_org(&data.organizationId, &conn);
    let ciphers_json: Vec<Value> = ciphers.iter().map(|c| c.to_json(&headers.host, &headers.user.uuid, &conn)).collect();

    Ok(Json(json!({
      "Data": ciphers_json,
      "Object": "list",
    })))
}

#[get("/organizations/<org_id>/users")]
fn get_org_users(org_id: String, headers: AdminHeaders, conn: DbConn) -> JsonResult {
    match UserOrganization::find_by_user_and_org(&headers.user.uuid, &org_id, &conn) {
        Some(_) => (),
        None => err!("User isn't member of organization")
    }

    let users = UserOrganization::find_by_org(&org_id, &conn);
    let users_json: Vec<Value> = users.iter().map(|c| c.to_json_user_details(&conn)).collect();

    Ok(Json(json!({
        "Data": users_json,
        "Object": "list"
    })))
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct CollectionData {
    id: String,
    readOnly: bool,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct InviteData {
    emails: Vec<String>,
    #[serde(rename = "type")]
    type_: NumberOrString,
    collections: Vec<CollectionData>,
    accessAll: Option<bool>,
}

#[post("/organizations/<org_id>/users/invite", data = "<data>")]
fn send_invite(org_id: String, data: Json<InviteData>, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let data: InviteData = data.into_inner();

    let new_type = match UserOrgType::from_str(&data.type_.to_string()) {
        Some(new_type) => new_type as i32,
        None => err!("Invalid type")
    };

    if new_type != UserOrgType::User as i32 &&
        headers.org_user_type != UserOrgType::Owner as i32 {
        err!("Only Owners can invite Admins or Owners")
    }

    for user_opt in data.emails.iter().map(|email| User::find_by_mail(email, &conn)) {
        match user_opt {
            None => err!("User email does not exist"),
            Some(user) => {
                match UserOrganization::find_by_user_and_org(&user.uuid, &org_id, &conn) {
                    Some(_) => err!("User already in organization"),
                    None => ()
                }

                let mut new_user = UserOrganization::new(user.uuid.clone(), org_id.clone());
                let access_all = data.accessAll.unwrap_or(false);
                new_user.access_all = access_all;
                new_user.type_ = new_type;

                // If no accessAll, add the collections received
                if !access_all {
                    for col in data.collections.iter() {
                        match Collection::find_by_uuid_and_org(&col.id, &org_id, &conn) {
                            None => err!("Collection not found in Organization"),
                            Some(collection) => {
                                match CollectionUser::save(&user.uuid, &collection.uuid, col.readOnly, &conn) {
                                    Ok(()) => (),
                                    Err(_) => err!("Failed saving collection access for user")
                                }
                            }
                        }
                    }
                }

                new_user.save(&conn);
            }
        }
    }

    Ok(())
}

#[post("/organizations/<org_id>/users/<user_id>/confirm", data = "<data>")]
fn confirm_invite(org_id: String, user_id: String, data: Json<Value>, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let mut user_to_confirm = match UserOrganization::find_by_uuid(&user_id, &conn) {
        Some(user) => user,
        None => err!("User to confirm doesn't exist")
    };

    if user_to_confirm.org_uuid != org_id {
        err!("The specified user isn't a member of the organization")
    }

    if user_to_confirm.type_ != UserOrgType::User as i32 &&
        headers.org_user_type != UserOrgType::Owner as i32 {
        err!("Only Owners can confirm Admins or Owners")
    }

    if user_to_confirm.status != UserOrgStatus::Accepted as i32 {
        err!("User in invalid state")
    }

    user_to_confirm.status = UserOrgStatus::Confirmed as i32;
    user_to_confirm.key = match data["key"].as_str() {
        Some(key) => key.to_string(),
        None => err!("Invalid key provided")
    };

    user_to_confirm.save(&conn);

    Ok(())
}

#[get("/organizations/<org_id>/users/<user_id>")]
fn get_user(org_id: String, user_id: String, _headers: AdminHeaders, conn: DbConn) -> JsonResult {
    let user = match UserOrganization::find_by_uuid(&user_id, &conn) {
        Some(user) => user,
        None => err!("The specified user doesn't exist")
    };

    if user.org_uuid != org_id {
        err!("The specified user isn't a member of the organization")
    }

    Ok(Json(user.to_json_details(&conn)))
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct EditUserData {
    #[serde(rename = "type")]
    type_: NumberOrString,
    collections: Vec<CollectionData>,
    accessAll: bool,
}

#[post("/organizations/<org_id>/users/<user_id>", data = "<data>", rank = 1)]
fn edit_user(org_id: String, user_id: String, data: Json<EditUserData>, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let data: EditUserData = data.into_inner();

    let new_type = match UserOrgType::from_str(&data.type_.to_string()) {
        Some(new_type) => new_type as i32,
        None => err!("Invalid type")
    };

    let mut user_to_edit = match UserOrganization::find_by_uuid(&user_id, &conn) {
        Some(user) => user,
        None => err!("The specified user isn't member of the organization")
    };

    if new_type != UserOrgType::User as i32 &&
        headers.org_user_type != UserOrgType::Owner as i32 {
        err!("Only Owners can grant Admin or Owner type")
    }

    if user_to_edit.type_ != UserOrgType::User as i32 &&
        headers.org_user_type != UserOrgType::Owner as i32 {
        err!("Only Owners can edit Admin or Owner")
    }

    if user_to_edit.type_ == UserOrgType::Owner as i32 &&
        new_type != UserOrgType::Owner as i32 {

        // Removing owner permmission, check that there are at least another owner
        let num_owners = UserOrganization::find_by_org_and_type(
            &org_id, UserOrgType::Owner as i32, &conn)
            .len();

        if num_owners <= 1 {
            err!("Can't delete the last owner")
        }
    }

    user_to_edit.access_all = data.accessAll;
    user_to_edit.type_ = new_type;

    // Delete all the odd collections
    for c in CollectionUser::find_by_organization_and_user_uuid(&org_id, &user_to_edit.user_uuid, &conn) {
        match c.delete(&conn) {
            Ok(()) => (),
            Err(_) => err!("Failed deleting old collection assignment")
        }
    }

    // If no accessAll, add the collections received
    if !data.accessAll {
        for col in data.collections.iter() {
            match Collection::find_by_uuid_and_org(&col.id, &org_id, &conn) {
                None => err!("Collection not found in Organization"),
                Some(collection) => {
                    match CollectionUser::save(&user_to_edit.user_uuid, &collection.uuid, col.readOnly, &conn) {
                        Ok(()) => (),
                        Err(_) => err!("Failed saving collection access for user")
                    }
                }
            }
        }
    }

    user_to_edit.save(&conn);

    Ok(())
}

#[post("/organizations/<org_id>/users/<user_id>/delete")]
fn delete_user(org_id: String, user_id: String, headers: AdminHeaders, conn: DbConn) -> EmptyResult {
    let user_to_delete = match UserOrganization::find_by_uuid(&user_id, &conn) {
        Some(user) => user,
        None => err!("User to delete isn't member of the organization")
    };

    if user_to_delete.type_ != UserOrgType::User as i32 &&
        headers.org_user_type != UserOrgType::Owner as i32 {
        err!("Only Owners can delete Admins or Owners")
    }

    if user_to_delete.type_ == UserOrgType::Owner as i32 {
        // Removing owner, check that there are at least another owner
        let num_owners = UserOrganization::find_by_org_and_type(
            &org_id, UserOrgType::Owner as i32, &conn)
            .len();

        if num_owners <= 1 {
            err!("Can't delete the last owner")
        }
    }

    match user_to_delete.delete(&conn) {
        Ok(()) => Ok(()),
        Err(_) => err!("Failed deleting user from organization")
    }
}