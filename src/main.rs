use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use material::{diffuse_light::DiffuseLight, material::Material};
use model::{
    bvh::BvhNode,
    constant_medium::ConstantMedium,
    hit::{HitRecord, Hittable},
    moving_sphere::MovingSphere,
    r#box::Box,
    ray::Ray,
    rotate::RotateY,
    translate::Translate,
    vec3::Vec3,
    xy_rect::XyRect,
    xz_rect::XzRect,
    yz_rect::YzRect,
};
use Vec3 as Point3;

use rayon::prelude::{IntoParallelIterator, IntoParallelRefMutIterator, ParallelIterator};
use texture::{checker::CheckerTexture, image::ImageTexture, noise::NoiseTexture};
use util::{
    rtweekend::INFINITY,
    rtweekend::{random_double, random_double_by_range},
};

use crate::{
    material::{dielectric::Dielectric, lambertian::Lambertian, metal::Metal},
    model::{camera::Camera, color::Color, hit::HittableList, sphere::Sphere},
    util::rtweekend::PI,
};
mod material;
mod model;
mod texture;
mod util;

fn main() {
    // Image
    const ASPECT_RATIO: f64 = 1.0 / 1.0;
    const IMAGE_WIDTH: usize = 600;
    const SAMPLES_PER_PIXEL: usize = 100;
    const MAX_DEPTH: i32 = 50;
    const IMAGE_HEIGHT: usize = (IMAGE_WIDTH as f64 / ASPECT_RATIO) as usize;

    // World
    let world = cornell_box();
    let background = Vec3::new(0.0, 0.0, 0.0);

    // Camera
    let lookfrom = Point3::new(278.0, 278.0, -800.0);
    let lookat = Point3::new(278.0, 278.0, 0.0);
    let vup = Vec3::new(0.0, 1.0, 0.0);
    let dist_to_focus = 10.0;
    let aperture = 0.0;
    let vfov = 40.0;
    let time0 = 0.0;
    let time1 = 1.0;

    let camera = Camera::new(
        &lookfrom,
        &lookat,
        &vup,
        vfov,
        ASPECT_RATIO,
        aperture,
        dist_to_focus,
        time0,
        time1,
    );

    // Render
    print!("P3\n{} {}\n255\n", IMAGE_WIDTH, IMAGE_HEIGHT);

    let mut result = vec!["".to_owned(); IMAGE_WIDTH * IMAGE_HEIGHT];
    let result = Mutex::new(result);
    let mut v = vec![];
    for i in 0..IMAGE_WIDTH {
        v.push(i);
    }

    for j in (0..IMAGE_HEIGHT).rev() {
        eprint!("\rScanlines remaining: {} ", j);
        io::stderr().flush().unwrap();

        v.clone().into_par_iter().for_each(|x| {
            let mut pixel_color = Vec3::new(0.0, 0.0, 0.0);
            for s in 0..SAMPLES_PER_PIXEL {
                let u = (x as f64 + random_double()) / (IMAGE_WIDTH as f64 - 1.0);
                let v = (j as f64 + random_double()) / (IMAGE_HEIGHT as f64 - 1.0);
                let r = camera.get_ray(u, v);
                pixel_color += ray_color(&r, &background, &world, MAX_DEPTH);
            }

            let s = pixel_color.as_color_repr(SAMPLES_PER_PIXEL);
            let mut result = result.lock().unwrap();
            result[x * IMAGE_HEIGHT + j] = s.clone();
        });
    }

    let res = result.lock().unwrap();
    for j in (0..IMAGE_HEIGHT).rev() {
        for i in 0..IMAGE_WIDTH {
            let index = i * IMAGE_HEIGHT + j;
            print!("{}", res[index]);
        }
    }

    eprintln!("\nDone.");
}

fn ray_color(r: &Ray, background: &Vec3, world: &dyn Hittable, depth: i32) -> Vec3 {
    let mut rec = HitRecord::default();

    // If we've exceeded the ray bounce limit, no more light is gathered.
    if depth <= 0 {
        return Vec3::new(0.0, 0.0, 0.0);
    }

    // If the ray hits nothing, return the background color
    if !world.hit(r, 0.001, INFINITY, &mut rec) {
        return background.clone();
    }

    let mut scattered = Ray::new(&Vec3::new(0.0, 0.0, 0.0), &Vec3::new(0.0, 0.0, 0.0), 0.0);
    let mut attenuation = Vec3::new(0.0, 0.0, 0.0);
    let emitted = rec.material.emitted(rec.u, rec.v, &rec.p);

    if !rec
        .material
        .scatter(r, &rec, &mut attenuation, &mut scattered)
    {
        return emitted;
    }

    return emitted + attenuation * ray_color(&scattered, background, world, depth - 1);
}

fn random_scene() -> HittableList {
    let mut world = HittableList::new();

    let ground_material = Arc::new(Lambertian::new(&Vec3::new(0.5, 0.5, 0.5)));
    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, -1000.0, 0.0),
        1000.0,
        ground_material,
    )));

    let checker = Arc::new(CheckerTexture::new_with_color(
        &Vec3::new(0.2, 0.3, 0.1),
        &Vec3::new(0.9, 0.9, 0.9),
    ));
    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, -1000.0, 0.0),
        1000.0,
        Arc::new(Lambertian::new_with_texture(checker)),
    )));

    for a in -11..11 {
        for b in -11..11 {
            let choose_mat = random_double();
            let center = Point3::new(
                a as f64 + 0.9 * random_double(),
                0.2,
                b as f64 + 0.9 * random_double(),
            );

            if (center - Point3::new(4.0, 0.2, 0.0)).length() > 0.9 {
                let sphere_material: Arc<dyn Material + Sync + Send>;

                if choose_mat < 0.8 {
                    // diffuse
                    let albedo = Vec3::random() * Vec3::random();
                    sphere_material = Arc::new(Lambertian::new(&albedo));
                    let center2 = center + Vec3::new(0.0, random_double_by_range(0.0, 0.5), 0.0);
                    world.add(Arc::new(MovingSphere::new(
                        center,
                        center2,
                        0.0,
                        1.0,
                        0.2,
                        sphere_material,
                    )));
                } else if choose_mat < 0.95 {
                    // metal
                    let albedo = Vec3::random_by_range(0.5, 1.0);
                    let fuzz = random_double_by_range(0.0, 0.5);
                    sphere_material = Arc::new(Metal::new(&albedo, fuzz));
                    world.add(Arc::new(Sphere::new(center, 0.2, sphere_material)));
                } else {
                    // glass
                    sphere_material = Arc::new(Dielectric::new(1.5));
                    world.add(Arc::new(Sphere::new(center, 0.2, sphere_material)));
                }
            }
        }
    }

    let material1 = Arc::new(Dielectric::new(1.5));
    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, 1.0, 0.0),
        1.0,
        material1,
    )));

    let material2 = Arc::new(Lambertian::new(&Vec3::new(0.4, 0.2, 0.1)));
    world.add(Arc::new(Sphere::new(
        Point3::new(-4.0, 1.0, 0.0),
        1.0,
        material2,
    )));

    let material3 = Arc::new(Metal::new(&Vec3::new(0.7, 0.6, 0.5), 0.0));
    world.add(Arc::new(Sphere::new(
        Point3::new(4.0, 1.0, 0.0),
        1.0,
        material3,
    )));

    world
}

fn two_spheres() -> HittableList {
    let mut world = HittableList::new();

    let checker = Arc::new(CheckerTexture::new_with_color(
        &Vec3::new(0.2, 0.3, 0.1),
        &Vec3::new(0.9, 0.9, 0.9),
    ));

    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, -10.0, 0.0),
        10.0,
        Arc::new(Lambertian::new_with_texture(checker.clone())),
    )));

    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, 10.0, 0.0),
        10.0,
        Arc::new(Lambertian::new_with_texture(checker)),
    )));

    world
}

fn two_perlin_spheres() -> HittableList {
    let mut world = HittableList::new();

    let pertext = Arc::new(NoiseTexture::new(4.0));

    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, -1000.0, 0.0),
        1000.0,
        Arc::new(Lambertian::new_with_texture(pertext.clone())),
    )));

    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, 2.0, 0.0),
        2.0,
        Arc::new(Lambertian::new_with_texture(pertext)),
    )));

    world
}

fn earth() -> HittableList {
    let mut world = HittableList::new();

    let earth_texture = Arc::new(ImageTexture::new("earthmap.jpg".to_owned()));
    let earth_surface = Arc::new(Lambertian::new_with_texture(earth_texture));
    let globe = Arc::new(Sphere::new(Point3::new(0.0, 0.0, 0.0), 2.0, earth_surface));

    world.add(globe);

    world
}

fn simple_light() -> HittableList {
    let mut world = HittableList::new();

    let pertext = Arc::new(NoiseTexture::new(4.0));
    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, -1000.0, 0.0),
        1000.0,
        Arc::new(Lambertian::new_with_texture(pertext.clone())),
    )));
    world.add(Arc::new(Sphere::new(
        Point3::new(0.0, 2.0, 0.0),
        2.0,
        Arc::new(Lambertian::new_with_texture(pertext)),
    )));

    let difflight = Arc::new(DiffuseLight::new_with_color((Vec3::new(4.0, 4.0, 4.0))));
    world.add(Arc::new(XyRect::new(3.0, 5.0, 1.0, 4.0, -2.0, difflight)));

    world
}

fn cornell_box() -> HittableList {
    let mut world = HittableList::new();

    let red = Arc::new(Lambertian::new(&Vec3::new(0.65, 0.05, 0.05)));
    let white = Arc::new(Lambertian::new(&Vec3::new(0.73, 0.73, 0.73)));
    let green = Arc::new(Lambertian::new(&Vec3::new(0.12, 0.45, 0.15)));
    let light = Arc::new(DiffuseLight::new_with_color(Vec3::new(15.0, 15.0, 15.0)));

    world.add(Arc::new(YzRect::new(0.0, 555.0, 0.0, 555.0, 555.0, green)));
    world.add(Arc::new(YzRect::new(0.0, 555.0, 0.0, 555.0, 0.0, red)));
    world.add(Arc::new(XzRect::new(
        213.0, 343.0, 227.0, 332.0, 554.0, light,
    )));
    world.add(Arc::new(XzRect::new(
        0.0,
        555.0,
        0.0,
        555.0,
        0.0,
        white.clone(),
    )));
    world.add(Arc::new(XzRect::new(
        0.0,
        555.0,
        0.0,
        555.0,
        555.0,
        white.clone(),
    )));
    world.add(Arc::new(XyRect::new(
        0.0,
        555.0,
        0.0,
        555.0,
        555.0,
        white.clone(),
    )));

    // world.add(Arc::new(Box::new(
    //     &Point3::new(130.0, 0.0, 65.0),
    //     &Point3::new(295.0, 165.0, 230.0),
    //     white.clone(),
    // )));
    // world.add(Arc::new(Box::new(
    //     &Point3::new(265.0, 0.0, 295.0),
    //     &Point3::new(430.0, 330.0, 460.0),
    //     white.clone(),
    // )));

    let mut box1: Arc<dyn Hittable + Sync + Send> = Arc::new(Box::new(
        &Point3::new(0.0, 0.0, 0.0),
        &Point3::new(165.0, 330.0, 165.0),
        white.clone(),
    ));
    box1 = Arc::new(RotateY::new(box1, 15.0));
    box1 = Arc::new(Translate::new(box1, &Vec3::new(265.0, 0.0, 295.0)));
    world.add(box1);

    let mut box2: Arc<dyn Hittable + Sync + Send> = Arc::new(Box::new(
        &Point3::new(0.0, 0.0, 0.0),
        &Point3::new(165.0, 165.0, 165.0),
        white.clone(),
    ));
    box2 = Arc::new(RotateY::new(box2, -18.0));
    box2 = Arc::new(Translate::new(box2, &Vec3::new(130.0, 0.0, 65.0)));
    world.add(box2);

    world
}

fn cornell_smoke() -> HittableList {
    let mut world = HittableList::new();

    let red = Arc::new(Lambertian::new(&Vec3::new(0.65, 0.05, 0.05)));
    let white = Arc::new(Lambertian::new(&Vec3::new(0.73, 0.73, 0.73)));
    let green = Arc::new(Lambertian::new(&Vec3::new(0.12, 0.45, 0.15)));
    let light = Arc::new(DiffuseLight::new_with_color(Vec3::new(7.0, 7.0, 7.0)));

    world.add(Arc::new(YzRect::new(0.0, 555.0, 0.0, 555.0, 555.0, green)));
    world.add(Arc::new(YzRect::new(0.0, 555.0, 0.0, 555.0, 0.0, red)));
    world.add(Arc::new(XzRect::new(
        113.0, 443.0, 127.0, 432.0, 554.0, light,
    )));
    world.add(Arc::new(XzRect::new(
        0.0,
        555.0,
        0.0,
        555.0,
        0.0,
        white.clone(),
    )));
    world.add(Arc::new(XzRect::new(
        0.0,
        555.0,
        0.0,
        555.0,
        555.0,
        white.clone(),
    )));
    world.add(Arc::new(XyRect::new(
        0.0,
        555.0,
        0.0,
        555.0,
        555.0,
        white.clone(),
    )));

    let mut box1: Arc<dyn Hittable + Sync + Send> = Arc::new(Box::new(
        &Point3::new(0.0, 0.0, 0.0),
        &Point3::new(165.0, 330.0, 165.0),
        white.clone(),
    ));
    box1 = Arc::new(RotateY::new(box1, 15.0));
    box1 = Arc::new(Translate::new(box1, &Vec3::new(265.0, 0.0, 295.0)));
    world.add(box1.clone());

    let mut box2: Arc<dyn Hittable + Sync + Send> = Arc::new(Box::new(
        &Point3::new(0.0, 0.0, 0.0),
        &Point3::new(165.0, 165.0, 165.0),
        white.clone(),
    ));
    box2 = Arc::new(RotateY::new(box2, -18.0));
    box2 = Arc::new(Translate::new(box2, &Vec3::new(130.0, 0.0, 65.0)));
    world.add(box2.clone());

    world.add(Arc::new(ConstantMedium::new(
        box1,
        0.01,
        Vec3::new(0.0, 0.0, 1.0),
    )));
    world.add(Arc::new(ConstantMedium::new(
        box2,
        0.01,
        Vec3::new(1.0, 1.0, 1.0),
    )));

    world
}

fn final_scene() -> HittableList {
    let mut boxes1 = HittableList::new();
    let ground = Arc::new(Lambertian::new(&Vec3::new(0.48, 0.83, 0.53)));

    let boxes_per_side = 20;
    for i in 0..boxes_per_side {
        for j in 0..boxes_per_side {
            let w = 100.0;
            let x0 = -1000.0 + i as f64 * w;
            let z0 = -1000.0 + j as f64 * w;
            let y0 = 0.0;
            let x1 = x0 + w;
            let y1 = random_double_by_range(1.0, 101.0);
            let z1 = z0 + w;

            boxes1.add(Arc::new(Box::new(
                &Vec3::new(x0, y0, z0),
                &Vec3::new(x1, y1, z1),
                ground.clone(),
            )));
        }
    }

    let mut world = HittableList::new();

    world.add(Arc::new(BvhNode::new_with_list(&boxes1, 0.0, 1.0)));

    let light = Arc::new(DiffuseLight::new_with_color(Vec3::new(7.0, 7.0, 7.0)));
    world.add(Arc::new(XzRect::new(
        123.0,
        423.0,
        147.0,
        412.0,
        554.0,
        light.clone(),
    )));

    let center1 = Point3::new(400.0, 400.0, 200.0);
    let center2 = center1 + Point3::new(30.0, 0.0, 0.0);
    let moving_sphere_material = Arc::new(Lambertian::new(&Vec3::new(0.7, 0.3, 0.1)));
    world.add(Arc::new(MovingSphere::new(
        center1,
        center2,
        0.0,
        1.0,
        50.0,
        moving_sphere_material,
    )));

    world.add(Arc::new(Sphere::new(
        Vec3::new(260.0, 150.0, 45.0),
        50.0,
        Arc::new(Dielectric::new(1.5)),
    )));
    world.add(Arc::new(Sphere::new(
        Vec3::new(0.0, 150.0, 145.0),
        50.0,
        Arc::new(Metal::new(&Vec3::new(0.8, 0.8, 0.9), 1.0)),
    )));

    let mut boundary = Arc::new(Sphere::new(
        Point3::new(360.0, 150.0, 145.0),
        70.0,
        Arc::new(Dielectric::new(1.5)),
    ));
    world.add(boundary.clone());
    world.add(Arc::new(ConstantMedium::new(
        boundary.clone(),
        0.2,
        Vec3::new(0.2, 0.4, 0.9),
    )));
    boundary = Arc::new(Sphere::new(
        Point3::new(0.0, 0.0, 0.0),
        5000.0,
        Arc::new(Dielectric::new(1.5)),
    ));
    world.add(Arc::new(ConstantMedium::new(
        boundary.clone(),
        0.0001,
        Vec3::new(1.0, 1.0, 1.0),
    )));

    let emat = Arc::new(Lambertian::new_with_texture(Arc::new(ImageTexture::new(
        "earthmap.jpg".to_owned(),
    ))));
    world.add(Arc::new(Sphere::new(
        Point3::new(400.0, 200.0, 400.0),
        100.0,
        emat.clone(),
    )));
    let pertext = Arc::new(NoiseTexture::new(0.1));
    world.add(Arc::new(Sphere::new(
        Vec3::new(220.0, 280.0, 300.0),
        80.0,
        Arc::new(Lambertian::new_with_texture(pertext.clone())),
    )));

    let mut boxes2 = HittableList::new();
    let white = Arc::new(Lambertian::new(&Vec3::new(0.73, 0.73, 0.73)));
    let ns = 1000;
    for j in 0..ns {
        boxes2.add(Arc::new(Sphere::new(
            Point3::random_by_range(0.0, 165.0),
            10.0,
            white.clone(),
        )));
    }

    world.add(Arc::new(Translate::new(
        Arc::new(RotateY::new(
            Arc::new(BvhNode::new_with_list(&boxes2, 0.0, 1.0)),
            15.0,
        )),
        &Vec3::new(-100.0, 270.0, 395.0),
    )));

    world
}
